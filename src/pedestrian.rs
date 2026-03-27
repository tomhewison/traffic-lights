use std::time::Duration;

/// Manages pedestrian crossing requests, state, and the audible/visual alert.
///
/// This controller does NOT decide when to begin crossing — that decision
/// belongs to the Junction (which checks allRed first). This controller
/// only manages the crossing state machine.
pub struct PedestrianController {
    // TODO: add fields
    //   waiting: bool,
    //   crossing: bool,
    //   alert_active: bool,
    //   elapsed: Duration,
}

/// The duration for which pedestrians are held (all traffic on Red).
pub const PEDESTRIAN_HOLD_DURATION: Duration = Duration::from_secs(15);

impl PedestrianController {
    /// Creates a new controller with no pending requests and no active crossing.
    pub fn new() -> Self {
        unimplemented!()
    }

    /// Registers a pedestrian crossing request.
    pub fn request(&mut self) {
        unimplemented!()
    }

    /// Returns true if a pedestrian is waiting to cross.
    pub fn is_waiting(&self) -> bool {
        unimplemented!()
    }

    /// Returns true if pedestrians are currently crossing.
    pub fn is_crossing(&self) -> bool {
        unimplemented!()
    }

    /// Returns true if the pedestrian alert (audible + visual) is active.
    pub fn is_alert_active(&self) -> bool {
        unimplemented!()
    }

    /// Begins the pedestrian crossing phase.
    /// Precondition: a pedestrian must be waiting.
    /// Sets crossing = true, alert_active = true, resets elapsed.
    pub fn begin_crossing(&mut self) {
        unimplemented!()
    }

    /// Ends the pedestrian crossing phase.
    /// Resets crossing, waiting, and alert to false.
    pub fn end_crossing(&mut self) {
        unimplemented!()
    }

    /// Increments the crossing elapsed timer by dt.
    pub fn tick(&mut self, dt: Duration) {
        unimplemented!()
    }

    /// Returns true if the crossing is active and the hold duration has elapsed.
    pub fn should_end(&self) -> bool {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Initial state
    // =========================================================================

    #[test]
    fn new_controller_has_no_waiting() {
        let pc = PedestrianController::new();
        assert!(!pc.is_waiting());
    }

    #[test]
    fn new_controller_has_no_crossing() {
        let pc = PedestrianController::new();
        assert!(!pc.is_crossing());
    }

    #[test]
    fn new_controller_has_no_alert() {
        let pc = PedestrianController::new();
        assert!(!pc.is_alert_active());
    }

    // =========================================================================
    // Request
    // =========================================================================

    #[test]
    fn request_sets_waiting() {
        let mut pc = PedestrianController::new();
        pc.request();
        assert!(pc.is_waiting());
    }

    // =========================================================================
    // T-I5a | I5: Pedestrian indication | FUN-06: Pedestrian indication and alert
    // Initiate pedestrian crossing; check alert state.
    // Expected: pedAlert is true while pedCrossing is true.
    // =========================================================================
    #[test]
    fn t_i5a_alert_active_during_crossing() {
        let mut pc = PedestrianController::new();
        pc.request();
        pc.begin_crossing();
        assert!(pc.is_crossing());
        assert!(pc.is_alert_active());
    }

    // =========================================================================
    // T-I5b | I5: Pedestrian indication | FUN-06: Pedestrian indication and alert
    // End pedestrian crossing; check alert state.
    // Expected: pedAlert is false after crossing ends.
    // =========================================================================
    #[test]
    fn t_i5b_alert_inactive_after_crossing() {
        let mut pc = PedestrianController::new();
        pc.request();
        pc.begin_crossing();
        pc.end_crossing();
        assert!(!pc.is_alert_active());
        assert!(!pc.is_crossing());
        assert!(!pc.is_waiting());
    }

    // =========================================================================
    // T-H15 | Hoare: End pedestrian crossing after 15s hold
    // Expected: pedCrossing, pedWaiting, pedAlert all false; allRed true.
    // (allRed is checked at the Junction level)
    // =========================================================================
    #[test]
    fn t_h15_end_crossing_clears_all_state() {
        let mut pc = PedestrianController::new();
        pc.request();
        pc.begin_crossing();
        pc.tick(PEDESTRIAN_HOLD_DURATION);
        assert!(pc.should_end());

        pc.end_crossing();
        assert!(!pc.is_crossing());
        assert!(!pc.is_waiting());
        assert!(!pc.is_alert_active());
    }

    // =========================================================================
    // T-B4 | Boundary: Pedestrian hold for exactly 15s
    // Expected: Crossing ends at 15s; pedCrossing becomes false.
    // =========================================================================
    #[test]
    fn t_b4_should_end_at_exactly_15s() {
        let mut pc = PedestrianController::new();
        pc.request();
        pc.begin_crossing();

        // Just under 15s — should NOT end
        pc.tick(Duration::from_millis(14_999));
        assert!(!pc.should_end());

        // At exactly 15s — should end
        pc.tick(Duration::from_millis(1));
        assert!(pc.should_end());
    }

    // =========================================================================
    // Multiple rapid requests — supports T-S7
    // Only one crossing should occur per request batch.
    // =========================================================================
    #[test]
    fn multiple_requests_before_crossing_only_one_crossing() {
        let mut pc = PedestrianController::new();
        pc.request();
        pc.request(); // second press
        pc.request(); // third press

        pc.begin_crossing();
        assert!(pc.is_crossing());

        pc.end_crossing();
        // waiting should be cleared — no second crossing queued
        assert!(!pc.is_waiting());
    }
}
