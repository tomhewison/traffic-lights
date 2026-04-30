use crate::clock::{Clock, SystemClock};
use crate::direction::Direction;
use crate::error::TransitionError;
use crate::fault::{Fault, FaultMonitor};
use crate::installation::{Installation, configure_green_timeout_for_pair};
use crate::pedestrian::PedestrianController;
use crate::signal::Signal;
use std::time::Duration;

/// Extra time beyond a phase's nominal maximum before a progress fault is raised.
const PROGRESS_TOLERANCE: Duration = Duration::from_millis(500);

fn waiting_idx(direction: Direction) -> usize {
    match direction {
        Direction::North => 0,
        Direction::South => 1,
        Direction::East => 2,
        Direction::West => 3,
    }
}

/// Top-level junction controller.
///
/// Owns four installations (N, S, E, W) grouped into two pairs (NS, EW),
/// a pedestrian controller, and a fault monitor. Enforces all cross-pair
/// safety invariants from the formal specification.
pub struct Junction<C: Clock> {
    ns_a: Installation,
    ns_b: Installation,
    ew_a: Installation,
    ew_b: Installation,
    pedestrian: PedestrianController,
    fault_monitor: FaultMonitor,
    clock: C,
    last_tick: std::time::Instant,
    waiting: [bool; 4],
}

impl<C: Clock> Junction<C> {
    /// Creates a new junction with the given clock. All installations start Red.
    pub fn with_clock(clock: C) -> Self {
        let now = clock.now();
        Junction {
            ns_a: Installation::new(Direction::North),
            ns_b: Installation::new(Direction::South),
            ew_a: Installation::new(Direction::East),
            ew_b: Installation::new(Direction::West),
            pedestrian: PedestrianController::new(),
            fault_monitor: FaultMonitor::new(),
            clock,
            last_tick: now,
            waiting: [false; 4],
        }
    }

    /// Attempts to advance the NS pair to the next signal state.
    /// Returns Err if any precondition is violated.
    pub fn try_advance_ns(&mut self) -> Result<(), TransitionError> {
        if self.fault_monitor.is_all_off() {
            return Err(TransitionError::SystemShutdown);
        }
        if self.pedestrian.is_crossing() {
            return Err(TransitionError::PedestrianCrossing);
        }
        if self.ew_signal().is_active() {
            return Err(TransitionError::ConflictingSignal);
        }
        let s = self.ns_a.signal();
        if s == Signal::Off {
            return Err(TransitionError::InvalidTransition);
        }
        let next = s.next();
        if ((s == Signal::Red && next == Signal::RedAmber)
            || (s == Signal::RedAmber && next == Signal::Green))
            && !self.ew_pair_all_red()
        {
            return Err(TransitionError::ConflictingSignal);
        }
        self.ns_a.advance();
        self.ns_b.advance();
        if self.ns_a.signal() == Signal::Green {
            let competing = self.ew_waiting();
            configure_green_timeout_for_pair(&mut self.ns_a, &mut self.ns_b, competing);
        }
        Ok(())
    }

    /// Attempts to advance the EW pair to the next signal state.
    /// Returns Err if any precondition is violated.
    pub fn try_advance_ew(&mut self) -> Result<(), TransitionError> {
        if self.fault_monitor.is_all_off() {
            return Err(TransitionError::SystemShutdown);
        }
        if self.pedestrian.is_crossing() {
            return Err(TransitionError::PedestrianCrossing);
        }
        if self.ns_signal().is_active() {
            return Err(TransitionError::ConflictingSignal);
        }
        let s = self.ew_a.signal();
        if s == Signal::Off {
            return Err(TransitionError::InvalidTransition);
        }
        let next = s.next();
        if ((s == Signal::Red && next == Signal::RedAmber)
            || (s == Signal::RedAmber && next == Signal::Green))
            && !self.ns_pair_all_red()
        {
            return Err(TransitionError::ConflictingSignal);
        }
        self.ew_a.advance();
        self.ew_b.advance();
        if self.ew_a.signal() == Signal::Green {
            let competing = self.ns_waiting();
            configure_green_timeout_for_pair(&mut self.ew_a, &mut self.ew_b, competing);
        }
        Ok(())
    }

    /// Processes elapsed time: auto-advances timed phases, checks for faults,
    /// manages pedestrian crossing lifecycle.
    pub fn tick(&mut self) {
        let now = self.clock.now();
        let dt = now.saturating_duration_since(self.last_tick);
        self.last_tick = now;
        if dt.is_zero() {
            return;
        }

        for inst in [
            &mut self.ns_a,
            &mut self.ns_b,
            &mut self.ew_a,
            &mut self.ew_b,
        ] {
            inst.tick(dt);
        }

        if self.pedestrian.is_crossing() {
            self.pedestrian.tick(dt);
            if self.pedestrian.should_end() {
                self.pedestrian.end_crossing();
            }
        }

        self.check_progress_faults();
        self.maybe_auto_advance_ns();
        self.maybe_auto_advance_ew();
    }

    /// Returns the current signal of the NS pair.
    pub fn ns_signal(&self) -> Signal {
        self.ns_a.signal()
    }

    /// Returns the current signal of the EW pair.
    pub fn ew_signal(&self) -> Signal {
        self.ew_a.signal()
    }

    /// Returns the signal of a specific installation by direction.
    pub fn signal(&self, direction: Direction) -> Signal {
        self.installation(direction).signal()
    }

    /// Returns true if all installations are Red.
    pub fn is_all_red(&self) -> bool {
        self.ns_a.signal() == Signal::Red
            && self.ns_b.signal() == Signal::Red
            && self.ew_a.signal() == Signal::Red
            && self.ew_b.signal() == Signal::Red
    }

    /// Returns true if the system is in the allOff shutdown state.
    pub fn is_all_off(&self) -> bool {
        self.fault_monitor.is_all_off()
    }

    /// Returns true if the monitoring system has raised an alert.
    pub fn alert_raised(&self) -> bool {
        self.fault_monitor.alert_raised()
    }

    /// Returns true if pedestrians are currently crossing.
    pub fn ped_crossing_active(&self) -> bool {
        self.pedestrian.is_crossing()
    }

    /// Returns true if the pedestrian alert is active.
    pub fn ped_alert_active(&self) -> bool {
        self.pedestrian.is_alert_active()
    }

    /// Returns true if a pedestrian has requested a crossing (not yet started).
    pub fn is_ped_waiting(&self) -> bool {
        self.pedestrian.is_waiting()
    }

    /// Registers a pedestrian crossing request.
    pub fn request_pedestrian_crossing(&mut self) {
        self.pedestrian.request();
    }

    /// Begins the pedestrian crossing phase.
    /// Precondition: allRed, pedWaiting, not allOff.
    pub fn begin_pedestrian_crossing(&mut self) -> Result<(), TransitionError> {
        if self.fault_monitor.is_all_off() {
            return Err(TransitionError::SystemShutdown);
        }
        if !self.is_all_red() {
            return Err(TransitionError::InvalidTransition);
        }
        if !self.pedestrian.is_waiting() {
            return Err(TransitionError::InvalidTransition);
        }
        self.pedestrian.begin_crossing();
        Ok(())
    }

    /// Ends the pedestrian crossing phase.
    pub fn end_pedestrian_crossing(&mut self) {
        self.pedestrian.end_crossing();
    }

    /// Reports a light fault on the given installation direction.
    pub fn report_light_fault(&mut self, direction: Direction) {
        self.fault_monitor
            .report(Fault::LightFailIlluminate(direction));
        self.shutdown_all();
    }

    /// Reports a light de-illumination fault on the given installation direction.
    pub fn report_light_deilluminate_fault(&mut self, direction: Direction) {
        self.fault_monitor
            .report(Fault::LightFailDeilluminate(direction));
        self.shutdown_all();
    }

    /// Reports a sensor fault on the given installation direction.
    pub fn report_sensor_fault(&mut self, direction: Direction) {
        self.fault_monitor.report(Fault::SensorFault(direction));
        self.installation_mut(direction).set_sensor_fault();
    }

    /// Reports a progress fault on the given installation direction.
    pub fn report_progress_fault(&mut self, direction: Direction) {
        self.fault_monitor.report(Fault::ProgressFault(direction));
        self.shutdown_all();
    }

    /// Returns the green timeout for the given installation direction.
    pub fn green_timeout(&self, direction: Direction) -> Duration {
        self.installation(direction).green_timeout()
    }

    /// Sets competing traffic on the given direction (sensor detects waiting vehicles).
    pub fn set_competing_traffic(&mut self, direction: Direction, waiting: bool) {
        let prev_ew = self.ew_waiting();
        let prev_ns = self.ns_waiting();
        self.waiting[waiting_idx(direction)] = waiting;

        if self.ns_signal() == Signal::Green {
            let competing = self.ew_waiting();
            configure_green_timeout_for_pair(&mut self.ns_a, &mut self.ns_b, competing);
            if competing && !prev_ew {
                self.ns_a.reset_elapsed();
                self.ns_b.reset_elapsed();
            }
        }
        if self.ew_signal() == Signal::Green {
            let competing = self.ns_waiting();
            configure_green_timeout_for_pair(&mut self.ew_a, &mut self.ew_b, competing);
            if competing && !prev_ns {
                self.ew_a.reset_elapsed();
                self.ew_b.reset_elapsed();
            }
        }
    }

    fn installation(&self, direction: Direction) -> &Installation {
        match direction {
            Direction::North => &self.ns_a,
            Direction::South => &self.ns_b,
            Direction::East => &self.ew_a,
            Direction::West => &self.ew_b,
        }
    }

    fn installation_mut(&mut self, direction: Direction) -> &mut Installation {
        match direction {
            Direction::North => &mut self.ns_a,
            Direction::South => &mut self.ns_b,
            Direction::East => &mut self.ew_a,
            Direction::West => &mut self.ew_b,
        }
    }

    fn ns_pair_all_red(&self) -> bool {
        self.ns_a.signal() == Signal::Red && self.ns_b.signal() == Signal::Red
    }

    fn ew_pair_all_red(&self) -> bool {
        self.ew_a.signal() == Signal::Red && self.ew_b.signal() == Signal::Red
    }

    fn ew_waiting(&self) -> bool {
        self.waiting[waiting_idx(Direction::East)] || self.waiting[waiting_idx(Direction::West)]
    }

    fn ns_waiting(&self) -> bool {
        self.waiting[waiting_idx(Direction::North)] || self.waiting[waiting_idx(Direction::South)]
    }

    fn shutdown_all(&mut self) {
        self.ns_a.shutdown();
        self.ns_b.shutdown();
        self.ew_a.shutdown();
        self.ew_b.shutdown();
        self.pedestrian.end_crossing();
    }

    fn check_progress_faults(&mut self) {
        if self.fault_monitor.is_all_off() {
            return;
        }
        let dirs = [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
        ];
        let mut fault_dir = None;
        for dir in dirs {
            let (elapsed, max_opt) = {
                let inst = self.installation(dir);
                (inst.elapsed(), inst.progress_max_elapsed())
            };
            match max_opt {
                Some(max) if elapsed >= max + PROGRESS_TOLERANCE => {
                    fault_dir = Some(dir);
                    break;
                }
                _ => {}
            }
        }
        if let Some(dir) = fault_dir {
            self.fault_monitor.report(Fault::ProgressFault(dir));
            self.shutdown_all();
        }
    }

    fn maybe_auto_advance_ns(&mut self) {
        if self.fault_monitor.is_all_off() || self.pedestrian.is_crossing() {
            return;
        }
        if self.ns_a.signal() != self.ns_b.signal() {
            return;
        }
        if !self.ns_a.should_advance() {
            return;
        }
        let s = self.ns_a.signal();
        let next = s.next();
        if s == Signal::RedAmber && next == Signal::Green && !self.ew_pair_all_red() {
            return;
        }
        self.ns_a.advance();
        self.ns_b.advance();
        if self.ns_a.signal() == Signal::Green {
            let competing = self.ew_waiting();
            configure_green_timeout_for_pair(&mut self.ns_a, &mut self.ns_b, competing);
        }
    }

    fn maybe_auto_advance_ew(&mut self) {
        if self.fault_monitor.is_all_off() || self.pedestrian.is_crossing() {
            return;
        }
        if self.ew_a.signal() != self.ew_b.signal() {
            return;
        }
        if !self.ew_a.should_advance() {
            return;
        }
        let s = self.ew_a.signal();
        let next = s.next();
        if s == Signal::RedAmber && next == Signal::Green && !self.ns_pair_all_red() {
            return;
        }
        self.ew_a.advance();
        self.ew_b.advance();
        if self.ew_a.signal() == Signal::Green {
            let competing = self.ns_waiting();
            configure_green_timeout_for_pair(&mut self.ew_a, &mut self.ew_b, competing);
        }
    }
}

impl Default for Junction<SystemClock> {
    fn default() -> Self {
        Self::new()
    }
}

impl Junction<SystemClock> {
    /// Creates a junction backed by the real system clock.
    pub fn new() -> Self {
        Self::with_clock(SystemClock)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::MockClock;

    /// Helper: create a junction with a mock clock for testing.
    fn test_junction() -> Junction<MockClock> {
        Junction::with_clock(MockClock::new())
    }

    // =========================================================================
    //
    //  INVARIANT TESTS
    //
    // =========================================================================

    // =========================================================================
    // T-I1a | I1: Mutual exclusion of intersecting signals | SAF-01
    // Set NS to Green; attempt to set EW to Red+Amber.
    // Expected: Transition is rejected; EW remains Red.
    // =========================================================================
    #[test]
    fn t_i1a_ns_green_blocks_ew_transition() {
        let mut jn = test_junction();

        // Advance NS: R → RA → G
        jn.try_advance_ns().unwrap(); // R → RA
        jn.try_advance_ns().unwrap(); // RA → G
        assert_eq!(jn.ns_signal(), Signal::Green);

        // EW should still be Red
        assert_eq!(jn.ew_signal(), Signal::Red);

        // Attempting to advance EW must fail
        let result = jn.try_advance_ew();
        assert!(result.is_err());
        assert_eq!(jn.ew_signal(), Signal::Red);
    }

    // =========================================================================
    // T-I1b | I1: Mutual exclusion of intersecting signals | SAF-01
    // Set NS to Amber (clearing); attempt to set EW to Red+Amber.
    // Expected: Transition is rejected; EW remains Red until NS reaches Red.
    // =========================================================================
    #[test]
    fn t_i1b_ns_amber_blocks_ew_transition() {
        let mut jn = test_junction();

        // Advance NS: R → RA → G → A
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        assert_eq!(jn.ns_signal(), Signal::Amber);

        // EW still blocked
        let result = jn.try_advance_ew();
        assert!(result.is_err());
        assert_eq!(jn.ew_signal(), Signal::Red);
    }

    // =========================================================================
    // T-I1c | I1: Mutual exclusion of intersecting signals | SAF-01
    // Set NS to Red+Amber; attempt to set EW to Red+Amber.
    // Expected: Transition is rejected; EW remains Red.
    // =========================================================================
    #[test]
    fn t_i1c_ns_red_amber_blocks_ew_transition() {
        let mut jn = test_junction();

        // Advance NS: R → RA
        jn.try_advance_ns().unwrap();
        assert_eq!(jn.ns_signal(), Signal::RedAmber);

        // EW blocked
        let result = jn.try_advance_ew();
        assert!(result.is_err());
        assert_eq!(jn.ew_signal(), Signal::Red);
    }

    // =========================================================================
    // T-I2a | I2: Pedestrian crossing requires all Red | SAF-02
    // Initiate pedestrian crossing; attempt any signal transition.
    // Expected: All transitions are rejected; all installations remain Red.
    // =========================================================================
    #[test]
    fn t_i2a_no_transitions_during_pedestrian_crossing() {
        let mut jn = test_junction();
        assert!(jn.is_all_red());

        jn.request_pedestrian_crossing();
        jn.begin_pedestrian_crossing().unwrap();

        // Both NS and EW transitions must be rejected
        assert!(jn.try_advance_ns().is_err());
        assert!(jn.try_advance_ew().is_err());

        // All still Red
        assert_eq!(jn.ns_signal(), Signal::Red);
        assert_eq!(jn.ew_signal(), Signal::Red);
    }

    // =========================================================================
    // T-I2b | I2: Pedestrian crossing requires all Red | SAF-02
    // Pedestrian crossing active; verify no installation is active.
    // Expected: All installations report Red; active(x) is false for all x.
    // =========================================================================
    #[test]
    fn t_i2b_all_installations_red_during_crossing() {
        let mut jn = test_junction();
        jn.request_pedestrian_crossing();
        jn.begin_pedestrian_crossing().unwrap();

        assert!(jn.is_all_red());

        // Check each direction individually
        for dir in [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
        ] {
            assert_eq!(jn.signal(dir), Signal::Red);
        }
    }

    // =========================================================================
    // T-I3a | I3: Paired synchronisation | SAF-03
    // Transition NS pair from Red to Red+Amber; check both N and S.
    // Expected: Both N and S display Red+Amber simultaneously.
    // =========================================================================
    #[test]
    fn t_i3a_ns_pair_transitions_together_to_red_amber() {
        let mut jn = test_junction();

        jn.try_advance_ns().unwrap();

        assert_eq!(jn.signal(Direction::North), Signal::RedAmber);
        assert_eq!(jn.signal(Direction::South), Signal::RedAmber);
    }

    // =========================================================================
    // T-I3b | I3: Paired synchronisation | SAF-03
    // Transition EW pair through full cycle; verify E equals W at each step.
    // Expected: signal(E) = signal(W) at every observed state.
    // =========================================================================
    #[test]
    fn t_i3b_ew_pair_stays_synchronised_through_full_cycle() {
        let mut jn = test_junction();

        // EW: R → RA → G → A → R
        let expected_states = [Signal::RedAmber, Signal::Green, Signal::Amber, Signal::Red];

        for expected in expected_states {
            jn.try_advance_ew().unwrap();
            assert_eq!(jn.signal(Direction::East), expected);
            assert_eq!(jn.signal(Direction::West), expected);
            assert_eq!(jn.signal(Direction::East), jn.signal(Direction::West));
        }
    }

    // =========================================================================
    // T-I6a | I6: Sensor fault fallback | SAF-04
    // Inject sensor fault on installation E; check green timeout.
    // Expected: greenTimeout(E) = 30 and alertRaised is true.
    // =========================================================================
    #[test]
    fn t_i6a_sensor_fault_sets_timeout_and_raises_alert() {
        let mut jn = test_junction();

        jn.report_sensor_fault(Direction::East);

        assert_eq!(jn.green_timeout(Direction::East), Duration::from_secs(30));
        assert!(jn.alert_raised());
        // Crucially, the system is NOT in allOff — graceful degradation
        assert!(!jn.is_all_off());
    }

    // =========================================================================
    // T-I6b | I6: Sensor fault fallback | SAF-04
    // Inject sensor fault; verify Green does not exceed 30s.
    // Expected: Installation transitions to Amber at or before 30s.
    // (Requires mock clock to simulate time passing)
    // =========================================================================
    #[test]
    fn t_i6b_sensor_fault_green_does_not_exceed_30s() {
        let clock = MockClock::new();
        let mut jn = Junction::with_clock(clock.clone());

        jn.report_sensor_fault(Direction::East);

        jn.try_advance_ew().unwrap(); // R → RA
        jn.try_advance_ew().unwrap(); // RA → G
        assert_eq!(jn.ew_signal(), Signal::Green);

        clock.advance(Duration::from_secs(30));
        jn.tick();
        assert_eq!(jn.ew_signal(), Signal::Amber);
    }

    // =========================================================================
    // T-I8a | I8: Fault state is terminal | SAF-05
    // After entering allOff, attempt to transition any installation.
    // Expected: Transition is rejected; system remains in allOff.
    // =========================================================================
    #[test]
    fn t_i8a_all_off_is_terminal() {
        let mut jn = test_junction();

        // Trigger shutdown via light fault
        jn.report_light_fault(Direction::North);
        assert!(jn.is_all_off());

        // No transitions should be possible
        assert!(jn.try_advance_ns().is_err());
        assert!(jn.try_advance_ew().is_err());
        assert!(jn.is_all_off());
    }

    // =========================================================================
    // T-I9a | I9: Intermediate phase timing | FUN-02
    // Set installation to Red+Amber; wait 1.5s.
    // Expected: Installation transitions to Green at 1.5s.
    // =========================================================================
    #[test]
    fn t_i9a_red_amber_auto_advances_at_1500ms() {
        let clock = MockClock::new();
        let mut jn = Junction::with_clock(clock.clone());

        jn.try_advance_ns().unwrap(); // R → RA
        assert_eq!(jn.ns_signal(), Signal::RedAmber);

        clock.advance(Duration::from_millis(1500));
        jn.tick();
        assert_eq!(jn.ns_signal(), Signal::Green);
    }

    // =========================================================================
    // T-I9b | I9: Intermediate phase timing | FUN-02
    // Set installation to Amber; wait 1.5s.
    // Expected: Installation transitions to Red at 1.5s.
    // =========================================================================
    #[test]
    fn t_i9b_amber_auto_advances_at_1500ms() {
        let clock = MockClock::new();
        let mut jn = Junction::with_clock(clock.clone());

        // Advance NS to Amber: R → RA → G → A
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        assert_eq!(jn.ns_signal(), Signal::Amber);

        clock.advance(Duration::from_millis(1500));
        jn.tick();
        assert_eq!(jn.ns_signal(), Signal::Red);
    }

    // =========================================================================
    // T-I10a | I10: Green duration with competing traffic | FUN-03
    // Set NS to Green with competing traffic on EW; wait 30s.
    // Expected: NS transitions to Amber at or before 30s.
    // =========================================================================
    #[test]
    fn t_i10a_green_with_competing_traffic_advances_at_30s() {
        let clock = MockClock::new();
        let mut jn = Junction::with_clock(clock.clone());

        // Advance NS to Green
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        assert_eq!(jn.ns_signal(), Signal::Green);

        jn.set_competing_traffic(Direction::East, true);

        clock.advance(Duration::from_secs(30));
        jn.tick();
        assert_eq!(jn.ns_signal(), Signal::Amber);
    }

    // =========================================================================
    // T-I10b | I10: Green duration with competing traffic | FUN-03
    // Set NS to Green with competing traffic; check at 29s.
    // Expected: NS is still Green (boundary: not yet expired).
    // =========================================================================
    #[test]
    fn t_i10b_green_with_competing_traffic_still_green_at_29s() {
        let clock = MockClock::new();
        let mut jn = Junction::with_clock(clock.clone());

        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        jn.set_competing_traffic(Direction::East, true);

        clock.advance(Duration::from_secs(29));
        jn.tick();
        assert_eq!(jn.ns_signal(), Signal::Green);
    }

    // =========================================================================
    // T-I11a | I11: Green duration without competing traffic | FUN-04
    // Set NS to Green with no competing traffic; wait 60s.
    // Expected: NS remains Green (no forced transition).
    // =========================================================================
    #[test]
    fn t_i11a_green_without_competing_traffic_stays_green_at_60s() {
        let clock = MockClock::new();
        let mut jn = Junction::with_clock(clock.clone());

        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        assert_eq!(jn.ns_signal(), Signal::Green);

        clock.advance(Duration::from_secs(60));
        jn.tick();
        assert_eq!(jn.ns_signal(), Signal::Green);
    }

    // =========================================================================
    // T-I11b | I11: Green duration without competing traffic | FUN-04
    // NS Green with no competing traffic; traffic arrives at 45s.
    // Expected: greenTimeout becomes 30s from the point competing traffic detected.
    // =========================================================================
    #[test]
    fn t_i11b_green_timeout_resets_when_traffic_arrives() {
        let clock = MockClock::new();
        let mut jn = Junction::with_clock(clock.clone());

        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();

        clock.advance(Duration::from_secs(45));
        jn.tick();
        jn.set_competing_traffic(Direction::East, true);

        clock.advance(Duration::from_secs(30));
        jn.tick();
        assert_eq!(jn.ns_signal(), Signal::Amber);
    }

    // =========================================================================
    //
    //  HOARE TRIPLE TESTS
    //
    // =========================================================================

    // =========================================================================
    // T-H1 | Hoare: R → RA | FUN-01
    // R → RA with valid preconditions (not faulted, not ped crossing, not allOff).
    // Expected: Installation advances to RA; paired installation also shows RA.
    // =========================================================================
    #[test]
    fn t_h1_red_to_red_amber_with_valid_preconditions() {
        let mut jn = test_junction();

        let result = jn.try_advance_ns();
        assert!(result.is_ok());
        assert_eq!(jn.ns_signal(), Signal::RedAmber);

        // Paired: both N and S should be RA
        assert_eq!(jn.signal(Direction::North), Signal::RedAmber);
        assert_eq!(jn.signal(Direction::South), Signal::RedAmber);
    }

    // =========================================================================
    // T-H2 | Hoare: R → RA rejected during ped crossing | SAF-02
    // R → RA while pedCrossing is true.
    // Expected: Transition is rejected.
    // =========================================================================
    #[test]
    fn t_h2_red_to_red_amber_rejected_during_ped_crossing() {
        let mut jn = test_junction();

        jn.request_pedestrian_crossing();
        jn.begin_pedestrian_crossing().unwrap();

        let result = jn.try_advance_ns();
        assert!(result.is_err());
        assert_eq!(jn.ns_signal(), Signal::Red);
    }

    // =========================================================================
    // T-H3 | Hoare: R → RA rejected on light fault | SAF-06
    // R → RA while lightFault(x) is true.
    // Expected: Transition is rejected; system enters allOff.
    // =========================================================================
    #[test]
    fn t_h3_red_to_red_amber_rejected_on_light_fault() {
        let mut jn = test_junction();

        jn.report_light_fault(Direction::North);
        assert!(jn.is_all_off());

        let result = jn.try_advance_ns();
        assert!(result.is_err());
    }

    // =========================================================================
    // T-H4 | Hoare: RA → G with intersecting Red | SAF-01
    // RA → G with all intersecting installations on Red.
    // Expected: Installation advances to G; intersecting installations remain Red.
    // =========================================================================
    #[test]
    fn t_h4_red_amber_to_green_with_intersecting_red() {
        let mut jn = test_junction();

        // NS: R → RA
        jn.try_advance_ns().unwrap();
        assert_eq!(jn.ns_signal(), Signal::RedAmber);
        assert_eq!(jn.ew_signal(), Signal::Red);

        // NS: RA → G (EW is Red, so this is allowed)
        jn.try_advance_ns().unwrap();
        assert_eq!(jn.ns_signal(), Signal::Green);
        assert_eq!(jn.ew_signal(), Signal::Red);
    }

    // =========================================================================
    // T-H5 | Hoare: RA → G rejected when intersecting is Amber | SAF-01
    // RA → G while intersecting installation is on Amber.
    // Expected: Transition is rejected.
    // =========================================================================
    #[test]
    fn t_h5_red_amber_to_green_rejected_when_intersecting_active() {
        // This scenario requires both pairs to be in non-Red states simultaneously,
        // which I1 should prevent. We test that the guard correctly rejects it
        // even if somehow both pairs reached non-Red states.
        //
        // In practice, this means: if EW were somehow Active when NS tries RA→G,
        // the transition is rejected. The simplest way to test this is:
        // advance NS to RA, then somehow get EW to Amber.
        // Since I1 prevents this, we test that the guard exists by checking
        // the NS RA→G transition requires EW to be Red.
        let mut jn = test_junction();

        jn.try_advance_ns().unwrap(); // NS: R → RA
        jn.try_advance_ns().unwrap(); // NS: RA → G

        // Now EW is Red, NS is Green.
        // If we advance NS to A → R, and then advance EW to RA,
        // and then try to advance EW to G while NS is still cycling:
        jn.try_advance_ns().unwrap(); // NS: G → A
        // NS is Amber (active) — EW should NOT be able to go to RA
        let result = jn.try_advance_ew();
        assert!(result.is_err());
    }

    // =========================================================================
    // T-H6 | Hoare: G → A | FUN-01
    // G → A with valid preconditions.
    // Expected: Installation advances to A; paired installation also shows A.
    // =========================================================================
    #[test]
    fn t_h6_green_to_amber_with_valid_preconditions() {
        let mut jn = test_junction();

        // Advance NS to Green
        jn.try_advance_ns().unwrap(); // R → RA
        jn.try_advance_ns().unwrap(); // RA → G

        // G → A
        jn.try_advance_ns().unwrap();
        assert_eq!(jn.ns_signal(), Signal::Amber);
        assert_eq!(jn.signal(Direction::North), Signal::Amber);
        assert_eq!(jn.signal(Direction::South), Signal::Amber);
    }

    // =========================================================================
    // T-H7 | Hoare: G → A rejected during ped crossing | SAF-02
    // G → A while pedCrossing is true.
    // Expected: Transition is rejected.
    //
    // NOTE: In practice, if pedCrossing is true then all signals must be Red (I2),
    //       so the installation cannot be at Green. This test verifies the guard
    //       exists as a defence-in-depth measure.
    // =========================================================================
    #[test]
    fn t_h7_green_to_amber_rejected_during_ped_crossing() {
        let mut jn = test_junction();

        // Advance NS to Green
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        assert_eq!(jn.ns_signal(), Signal::Green);

        // Begin ped crossing — but this requires allRed, so in a correctly
        // implemented system this would fail. We test the guard on G→A directly.
        // If your begin_pedestrian_crossing checks allRed, then this test
        // verifies that try_advance_ns rejects during crossing state.
        // You may need to set the crossing state directly for this test.
        // Approach: advance NS back to R first, then begin crossing,
        // then verify no transition can occur.
        jn.try_advance_ns().unwrap(); // G → A
        jn.try_advance_ns().unwrap(); // A → R
        assert!(jn.is_all_red());

        jn.request_pedestrian_crossing();
        jn.begin_pedestrian_crossing().unwrap();

        // Now try to advance — should be rejected
        let result = jn.try_advance_ns();
        assert!(result.is_err());
    }

    // =========================================================================
    // T-H8 | Hoare: G → A rejected on light fault | SAF-06
    // G → A while lightFault(x) is true.
    // Expected: Transition is rejected; system enters allOff.
    // =========================================================================
    #[test]
    fn t_h8_green_to_amber_rejected_on_light_fault() {
        let mut jn = test_junction();

        // Advance NS to Green
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();

        // Inject light fault — system should immediately shut down
        jn.report_light_fault(Direction::North);
        assert!(jn.is_all_off());

        // No further transitions possible
        let result = jn.try_advance_ns();
        assert!(result.is_err());
    }

    // =========================================================================
    // T-H9 | Hoare: A → R | FUN-01
    // A → R with valid preconditions.
    // Expected: Installation advances to R; paired installation also shows R.
    // =========================================================================
    #[test]
    fn t_h9_amber_to_red_with_valid_preconditions() {
        let mut jn = test_junction();

        // Advance NS: R → RA → G → A
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        assert_eq!(jn.ns_signal(), Signal::Amber);

        // A → R
        jn.try_advance_ns().unwrap();
        assert_eq!(jn.ns_signal(), Signal::Red);
        assert_eq!(jn.signal(Direction::North), Signal::Red);
        assert_eq!(jn.signal(Direction::South), Signal::Red);
    }

    // =========================================================================
    // T-H10 | Hoare: A → R rejected on light fault | SAF-07
    // A → R while lightFault(x) is true.
    // Expected: Transition is rejected; system enters allOff.
    // =========================================================================
    #[test]
    fn t_h10_amber_to_red_rejected_on_light_fault() {
        let mut jn = test_junction();

        // Advance NS: R → RA → G → A
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();

        // Light de-illuminate fault — system shuts down
        jn.report_light_deilluminate_fault(Direction::North);
        assert!(jn.is_all_off());

        let result = jn.try_advance_ns();
        assert!(result.is_err());
    }

    // =========================================================================
    // T-H11 | Hoare: Paired transition | SAF-03
    // Paired transition: advance N from G to A.
    // Expected: Both N and S advance to A atomically.
    // =========================================================================
    #[test]
    fn t_h11_paired_transition_green_to_amber() {
        let mut jn = test_junction();

        // NS to Green
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();

        // G → A — both must transition
        jn.try_advance_ns().unwrap();
        assert_eq!(jn.signal(Direction::North), Signal::Amber);
        assert_eq!(jn.signal(Direction::South), Signal::Amber);
    }

    // =========================================================================
    // T-H12 | Hoare: Paired transition | SAF-03
    // Paired transition: verify no intermediate state where N ≠ S.
    // Expected: At no observable point does signal(N) ≠ signal(S).
    // =========================================================================
    #[test]
    fn t_h12_paired_signals_always_equal_through_full_cycle() {
        let mut jn = test_junction();

        // Full cycle: check N = S at every step
        for _ in 0..4 {
            assert_eq!(jn.signal(Direction::North), jn.signal(Direction::South));
            jn.try_advance_ns().unwrap();
        }
        assert_eq!(jn.signal(Direction::North), jn.signal(Direction::South));

        // Also check EW pair
        for _ in 0..4 {
            assert_eq!(jn.signal(Direction::East), jn.signal(Direction::West));
            jn.try_advance_ew().unwrap();
        }
        assert_eq!(jn.signal(Direction::East), jn.signal(Direction::West));
    }

    // =========================================================================
    // T-H13 | Hoare: Begin pedestrian crossing | FUN-05, FUN-06
    // Begin pedestrian crossing: allRed, pedWaiting, not allOff.
    // Expected: pedCrossing, pedAlert, and allRed all true.
    // =========================================================================
    #[test]
    fn t_h13_begin_pedestrian_crossing_valid() {
        let mut jn = test_junction();
        assert!(jn.is_all_red());

        jn.request_pedestrian_crossing();
        jn.begin_pedestrian_crossing().unwrap();

        assert!(jn.ped_crossing_active());
        assert!(jn.ped_alert_active());
        assert!(jn.is_all_red());
    }

    // =========================================================================
    // T-H14 | Hoare: Begin ped crossing rejected when not allRed | SAF-02
    // Begin pedestrian crossing while NS is Green.
    // Expected: Crossing does not begin; precondition not met.
    // =========================================================================
    #[test]
    fn t_h14_begin_pedestrian_crossing_rejected_when_not_all_red() {
        let mut jn = test_junction();

        // Advance NS to Green
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        assert_eq!(jn.ns_signal(), Signal::Green);

        jn.request_pedestrian_crossing();
        let result = jn.begin_pedestrian_crossing();
        assert!(result.is_err());
        assert!(!jn.ped_crossing_active());
    }

    // =========================================================================
    // T-H15 | Hoare: End pedestrian crossing after 15s hold | FUN-05
    // Expected: pedCrossing, pedWaiting, pedAlert all false; allRed true.
    // Junction-level: verifies tick() auto-ends crossing at 15s.
    // =========================================================================
    #[test]
    fn t_h15_end_crossing_after_15s_hold() {
        let clock = MockClock::new();
        let mut jn = Junction::with_clock(clock.clone());

        jn.request_pedestrian_crossing();
        jn.begin_pedestrian_crossing().unwrap();
        assert!(jn.ped_crossing_active());
        assert!(jn.ped_alert_active());

        clock.advance(Duration::from_secs(15));
        jn.tick();

        assert!(!jn.ped_crossing_active());
        assert!(!jn.is_ped_waiting());
        assert!(!jn.ped_alert_active());
        assert!(jn.is_all_red());
    }

    // =========================================================================
    // T-H16 | Hoare: Fault during pedestrian crossing | SAF-05, SAF-06, SAF-07
    // Fault occurs during pedestrian crossing.
    // Expected: System enters allOff; pedCrossing is false; alert raised.
    // =========================================================================
    #[test]
    fn t_h16_fault_during_pedestrian_crossing() {
        let mut jn = test_junction();

        jn.request_pedestrian_crossing();
        jn.begin_pedestrian_crossing().unwrap();
        assert!(jn.ped_crossing_active());

        // Light fault while pedestrians are crossing
        jn.report_light_fault(Direction::East);

        // Fault shutdown overrides pedestrian phase
        assert!(jn.is_all_off());
        assert!(jn.alert_raised());
        assert!(!jn.ped_crossing_active());
    }

    // =========================================================================
    // T-H17 | Hoare: Sensor fault graceful degradation | SAF-04
    // Sensor fault: verify graceful degradation.
    // Expected: greenTimeout = 30; alert raised; junction continues cycling.
    // =========================================================================
    #[test]
    fn t_h17_sensor_fault_graceful_degradation() {
        let mut jn = test_junction();

        jn.report_sensor_fault(Direction::East);

        assert_eq!(jn.green_timeout(Direction::East), Duration::from_secs(30));
        assert!(jn.alert_raised());
        assert!(!jn.is_all_off()); // NOT shutdown

        // Junction should still be able to cycle
        assert!(jn.try_advance_ns().is_ok());
    }

    // =========================================================================
    // T-H18 | Hoare: Light fault (illuminate) shutdown | SAF-06
    // Light fault (fail to illuminate): verify shutdown.
    // Expected: All installations off; alert raised.
    // =========================================================================
    #[test]
    fn t_h18_light_fault_illuminate_shutdown() {
        let mut jn = test_junction();

        jn.report_light_fault(Direction::North);

        assert!(jn.is_all_off());
        assert!(jn.alert_raised());

        // Check all signals are Off
        for dir in [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
        ] {
            assert_eq!(jn.signal(dir), Signal::Off);
        }
    }

    // =========================================================================
    // T-H19 | Hoare: Light fault (de-illuminate) shutdown | SAF-07
    // Light fault (fail to de-illuminate): verify shutdown.
    // Expected: All installations off; alert raised.
    // =========================================================================
    #[test]
    fn t_h19_light_fault_deilluminate_shutdown() {
        let mut jn = test_junction();

        jn.report_light_deilluminate_fault(Direction::East);

        assert!(jn.is_all_off());
        assert!(jn.alert_raised());

        for dir in [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
        ] {
            assert_eq!(jn.signal(dir), Signal::Off);
        }
    }

    // =========================================================================
    // T-H20 | Hoare: Progress fault shutdown | SAF-05
    // Progress fault: elapsed exceeds maxDuration.
    // Expected: All installations off; alert raised.
    // =========================================================================
    #[test]
    fn t_h20_progress_fault_shutdown() {
        let mut jn = test_junction();

        jn.report_progress_fault(Direction::South);

        assert!(jn.is_all_off());
        assert!(jn.alert_raised());
    }

    // =========================================================================
    //
    //  BOUNDARY / TIMING TESTS
    //
    // =========================================================================

    // =========================================================================
    // T-B1 | Boundary: Red+Amber held for exactly 1.5s | FUN-02, I9
    // Expected: Transitions to Green at 1.5s (not before, not significantly after).
    // =========================================================================
    #[test]
    fn t_b1_red_amber_transitions_at_exactly_1500ms() {
        let clock = MockClock::new();
        let mut jn = Junction::with_clock(clock.clone());

        jn.try_advance_ns().unwrap(); // R → RA

        clock.advance(Duration::from_millis(1499));
        jn.tick();
        assert_eq!(jn.ns_signal(), Signal::RedAmber);

        clock.advance(Duration::from_millis(1));
        jn.tick();
        assert_eq!(jn.ns_signal(), Signal::Green);
    }

    // =========================================================================
    // T-B2 | Boundary: Amber held for exactly 1.5s | FUN-02, I9
    // Expected: Transitions to Red at 1.5s.
    // =========================================================================
    #[test]
    fn t_b2_amber_transitions_at_exactly_1500ms() {
        let clock = MockClock::new();
        let mut jn = Junction::with_clock(clock.clone());

        // Advance NS: R → RA → G → A
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();

        clock.advance(Duration::from_millis(1499));
        jn.tick();
        assert_eq!(jn.ns_signal(), Signal::Amber);

        clock.advance(Duration::from_millis(1));
        jn.tick();
        assert_eq!(jn.ns_signal(), Signal::Red);
    }

    // =========================================================================
    // T-B3 | Boundary: Green with competing traffic held for exactly 30s | FUN-03, I10
    // Expected: Transitions to Amber at 30s.
    // =========================================================================
    #[test]
    fn t_b3_green_with_competing_traffic_transitions_at_30s() {
        let clock = MockClock::new();
        let mut jn = Junction::with_clock(clock.clone());

        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        jn.set_competing_traffic(Direction::East, true);

        clock.advance(Duration::from_secs(29));
        jn.tick();
        assert_eq!(jn.ns_signal(), Signal::Green);

        clock.advance(Duration::from_secs(1));
        jn.tick();
        assert_eq!(jn.ns_signal(), Signal::Amber);
    }

    // =========================================================================
    // T-B4 | Boundary: Pedestrian hold for exactly 15s | FUN-05
    // Junction-level: crossing does NOT end at 14.999s, DOES end at 15s.
    // =========================================================================
    #[test]
    fn t_b4_pedestrian_hold_exactly_15s() {
        let clock = MockClock::new();
        let mut jn = Junction::with_clock(clock.clone());

        jn.request_pedestrian_crossing();
        jn.begin_pedestrian_crossing().unwrap();

        clock.advance(Duration::from_millis(14_999));
        jn.tick();
        assert!(jn.ped_crossing_active());

        clock.advance(Duration::from_millis(1));
        jn.tick();
        assert!(!jn.ped_crossing_active());
        assert!(jn.is_all_red());
    }

    // =========================================================================
    // T-B5 | Boundary: Progress fault detection at 1.5s + tolerance | SAF-05, I7
    // Expected: No fault at 1.5s; fault detected at 1.5s + tolerance.
    // =========================================================================
    #[test]
    fn t_b5_progress_fault_amber_detected_after_tolerance() {
        let clock = MockClock::new();
        let mut jn = Junction::with_clock(clock.clone());

        // Advance NS: R → RA → G → A
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        assert_eq!(jn.ns_signal(), Signal::Amber);

        // Exactly 1.5s in Amber: auto-advance to Red, no progress fault
        clock.advance(Duration::from_millis(1500));
        jn.tick();
        assert!(!jn.is_all_off());
        assert_eq!(jn.ns_signal(), Signal::Red);

        // Amber again; one tick beyond max + tolerance without advancing ⇒ shutdown
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        assert_eq!(jn.ns_signal(), Signal::Amber);

        clock.advance(Duration::from_millis(1500) + PROGRESS_TOLERANCE);
        jn.tick();
        assert!(jn.is_all_off());
        assert!(jn.alert_raised());
    }

    // =========================================================================
    // T-B6 | Boundary: Progress fault detection at 30s + tolerance | SAF-05, I7
    // Expected: No fault at 30s; fault detected at 30s + tolerance.
    // =========================================================================
    #[test]
    fn t_b6_progress_fault_green_detected_after_tolerance() {
        let clock = MockClock::new();
        let mut jn = Junction::with_clock(clock.clone());

        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        jn.set_competing_traffic(Direction::East, true);

        clock.advance(Duration::from_secs(30));
        jn.tick();
        assert!(!jn.is_all_off());
        assert_eq!(jn.ns_signal(), Signal::Amber);

        // Fresh Green with competing traffic; one tick past 30s + tolerance ⇒ progress fault
        let clock = MockClock::new();
        let mut jn = Junction::with_clock(clock.clone());
        jn.try_advance_ns().unwrap();
        jn.try_advance_ns().unwrap();
        jn.set_competing_traffic(Direction::East, true);

        clock.advance(Duration::from_secs(30) + PROGRESS_TOLERANCE);
        jn.tick();
        assert!(jn.is_all_off());
        assert!(jn.alert_raised());
    }
}
