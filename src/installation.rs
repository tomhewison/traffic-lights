use crate::direction::Direction;
use crate::signal::Signal;
use std::time::Duration;

/// Default maximum green when competing traffic is present (no sensor fault).
const DEFAULT_COMPETING_GREEN: Duration = Duration::from_secs(30);

/// A single traffic light installation at one approach to the junction.
///
/// Holds the current signal state, elapsed time in that state, sensor status,
/// and green timeout. Does NOT enforce cross-installation invariants — that is
/// the Junction's responsibility.
pub struct Installation {
    direction: Direction,
    signal: Signal,
    elapsed: Duration,
    sensor_fault: bool,
    green_timeout: Duration,
}

impl Installation {
    /// Creates a new installation starting at Red with no faults.
    pub fn new(direction: Direction) -> Self {
        Installation {
            direction,
            signal: Signal::Red,
            elapsed: Duration::ZERO,
            sensor_fault: false,
            green_timeout: Duration::MAX,
        }
    }

    /// Returns the current signal state.
    pub fn signal(&self) -> Signal {
        self.signal
    }

    /// Returns the direction this installation faces.
    pub fn direction(&self) -> Direction {
        self.direction
    }

    /// Returns true if the signal is active (RA, G, or A).
    pub fn is_active(&self) -> bool {
        self.signal.is_active()
    }

    /// Advances to the next valid signal state. Resets elapsed to zero.
    pub fn advance(&mut self) {
        self.signal = self.signal.next();
        self.elapsed = Duration::ZERO;
    }

    /// Shuts down the installation (sets signal to Off).
    pub fn shutdown(&mut self) {
        self.signal = Signal::Off;
    }

    /// Returns the time elapsed since entering the current signal state.
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Increments the elapsed time by the given delta.
    pub fn tick(&mut self, dt: Duration) {
        self.elapsed += dt;
    }

    /// Returns true if the elapsed time exceeds the maximum duration
    /// for the current signal phase.
    pub fn should_advance(&self) -> bool {
        match self.signal {
            Signal::Red | Signal::Off => false,
            Signal::RedAmber | Signal::Amber => self
                .signal
                .max_duration()
                .is_some_and(|max| self.elapsed >= max),
            Signal::Green => {
                if self.sensor_fault {
                    self.elapsed >= Duration::from_secs(30)
                } else if self.green_limited() {
                    self.elapsed >= self.green_timeout
                } else {
                    false
                }
            }
        }
    }

    /// Reports a sensor fault. Sets green timeout to 30s.
    pub fn set_sensor_fault(&mut self) {
        self.sensor_fault = true;
        self.green_timeout = Duration::from_secs(30);
    }

    /// Returns true if the sensor has faulted.
    pub fn has_sensor_fault(&self) -> bool {
        self.sensor_fault
    }

    /// Returns the current green timeout duration.
    pub fn green_timeout(&self) -> Duration {
        self.green_timeout
    }

    /// Sets the maximum green duration for the current or next green phase (junction use).
    pub(crate) fn set_green_timeout(&mut self, d: Duration) {
        self.green_timeout = d;
    }

    /// Resets elapsed in the current phase (e.g. competing traffic arrived mid-green).
    pub(crate) fn reset_elapsed(&mut self) {
        self.elapsed = Duration::ZERO;
    }

    fn green_limited(&self) -> bool {
        !self.sensor_fault && self.green_timeout < Duration::MAX / 4
    }

    /// Maximum allowed duration in the current phase before a progress fault is raised (junction use).
    pub(crate) fn progress_max_elapsed(&self) -> Option<Duration> {
        match self.signal {
            Signal::RedAmber | Signal::Amber => Some(Duration::from_millis(1500)),
            Signal::Green => {
                if self.sensor_fault {
                    Some(Duration::from_secs(30))
                } else if self.green_limited() {
                    Some(self.green_timeout)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// Used when entering green from [`Installation::advance`]: junction sets limits on both pair members.
pub(crate) fn configure_green_timeout_for_pair(
    a: &mut Installation,
    b: &mut Installation,
    competing_on_intersecting: bool,
) {
    let limit = if a.has_sensor_fault() || b.has_sensor_fault() {
        Duration::from_secs(30)
    } else if competing_on_intersecting {
        DEFAULT_COMPETING_GREEN
    } else {
        Duration::MAX
    };
    a.set_green_timeout(limit);
    b.set_green_timeout(limit);
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Installation basics — construction and initial state
    // =========================================================================

    #[test]
    fn new_installation_starts_at_red() {
        let inst = Installation::new(Direction::North);
        assert_eq!(inst.signal(), Signal::Red);
    }

    #[test]
    fn new_installation_is_not_active() {
        let inst = Installation::new(Direction::North);
        assert!(!inst.is_active());
    }

    #[test]
    fn new_installation_has_zero_elapsed() {
        let inst = Installation::new(Direction::North);
        assert_eq!(inst.elapsed(), Duration::ZERO);
    }

    #[test]
    fn new_installation_has_no_sensor_fault() {
        let inst = Installation::new(Direction::North);
        assert!(!inst.has_sensor_fault());
    }

    #[test]
    fn new_installation_direction_is_correct() {
        let inst = Installation::new(Direction::East);
        assert_eq!(inst.direction(), Direction::East);
    }

    // =========================================================================
    // advance() — supports FUN-01 / I4
    // =========================================================================

    #[test]
    fn advance_from_red_to_red_amber() {
        let mut inst = Installation::new(Direction::North);
        inst.advance();
        assert_eq!(inst.signal(), Signal::RedAmber);
    }

    #[test]
    fn advance_resets_elapsed_to_zero() {
        let mut inst = Installation::new(Direction::North);
        inst.tick(Duration::from_secs(5));
        inst.advance();
        assert_eq!(inst.elapsed(), Duration::ZERO);
    }

    #[test]
    fn advance_through_full_cycle() {
        let mut inst = Installation::new(Direction::North);
        inst.advance(); // R → RA
        inst.advance(); // RA → G
        inst.advance(); // G → A
        inst.advance(); // A → R
        assert_eq!(inst.signal(), Signal::Red);
    }

    // =========================================================================
    // shutdown() — supports I7 (fault shutdown)
    // =========================================================================

    #[test]
    fn shutdown_sets_signal_to_off() {
        let mut inst = Installation::new(Direction::North);
        inst.shutdown();
        assert_eq!(inst.signal(), Signal::Off);
    }

    // =========================================================================
    // tick() and elapsed — supports timing invariants I9, I10
    // =========================================================================

    #[test]
    fn tick_increments_elapsed() {
        let mut inst = Installation::new(Direction::North);
        inst.tick(Duration::from_millis(500));
        assert_eq!(inst.elapsed(), Duration::from_millis(500));
    }

    #[test]
    fn tick_accumulates_over_multiple_calls() {
        let mut inst = Installation::new(Direction::North);
        inst.tick(Duration::from_millis(500));
        inst.tick(Duration::from_millis(700));
        assert_eq!(inst.elapsed(), Duration::from_millis(1200));
    }

    // =========================================================================
    // should_advance — supports auto-advance of timed phases
    // I9: RA and A are 1.5s
    // =========================================================================

    #[test]
    fn should_advance_false_at_red() {
        let inst = Installation::new(Direction::North);
        // Red has no fixed duration — should not auto-advance
        assert!(!inst.should_advance());
    }

    #[test]
    fn should_advance_true_when_red_amber_exceeds_1500ms() {
        let mut inst = Installation::new(Direction::North);
        inst.advance(); // R → RA
        inst.tick(Duration::from_millis(1500));
        assert!(inst.should_advance());
    }

    #[test]
    fn should_advance_false_when_red_amber_under_1500ms() {
        let mut inst = Installation::new(Direction::North);
        inst.advance(); // R → RA
        inst.tick(Duration::from_millis(1499));
        assert!(!inst.should_advance());
    }

    // =========================================================================
    // Sensor fault — supports I6 / SAF-04
    // =========================================================================

    // T-I6a | I6: Sensor fault fallback | SAF-04: Sensor failure
    // Inject sensor fault on installation E; check green timeout.
    // Expected: greenTimeout(E) = 30 and alertRaised is true.
    // (alertRaised is checked at the Junction level)
    #[test]
    fn t_i6a_sensor_fault_sets_green_timeout_to_30s() {
        let mut inst = Installation::new(Direction::East);
        inst.set_sensor_fault();
        assert_eq!(inst.green_timeout(), Duration::from_secs(30));
        assert!(inst.has_sensor_fault());
    }
}
