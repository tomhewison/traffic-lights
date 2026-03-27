use crate::direction::Direction;

/// Categorises the types of faults that may occur in the system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fault {
    /// A light failed to illuminate when commanded.
    LightFailIlluminate(Direction),
    /// A light failed to de-illuminate when commanded.
    LightFailDeilluminate(Direction),
    /// An installation did not progress to the next signal in time.
    ProgressFault(Direction),
    /// A traffic sensor on an installation has failed.
    SensorFault(Direction),
}

/// Tracks all faults and manages the system shutdown state.
pub struct FaultMonitor {
    all_off: bool,
    alert_raised: bool,
    faults: Vec<Fault>,
}

impl FaultMonitor {
    /// Creates a new fault monitor with no faults.
    pub fn new() -> Self {
        FaultMonitor {
            all_off: false,
            alert_raised: false,
            faults: Vec::new(),
        }
    }

    /// Reports a fault. Light and progress faults trigger a full shutdown (allOff).
    /// Sensor faults raise an alert but do NOT trigger shutdown.
    pub fn report(&mut self, fault: Fault) {
        match fault {
            Fault::LightFailIlluminate(_)
            | Fault::LightFailDeilluminate(_)
            | Fault::ProgressFault(_) => {
                self.all_off = true;
                self.alert_raised = true;
            }
            Fault::SensorFault(_) => {
                self.alert_raised = true;
            }
        }
        self.faults.push(fault);
    }

    /// Returns true if the system is in the allOff shutdown state.
    pub fn is_all_off(&self) -> bool {
        self.all_off
    }

    /// Returns true if the monitoring system has raised an alert.
    pub fn alert_raised(&self) -> bool {
        self.alert_raised
    }

    /// Returns true if any light fault (illuminate or de-illuminate) has been reported.
    pub fn has_light_fault(&self) -> bool {
        self.faults.iter().any(|f| {
            matches!(
                f,
                Fault::LightFailIlluminate(_) | Fault::LightFailDeilluminate(_)
            )
        })
    }

    /// Returns true if any progress fault has been reported.
    pub fn has_progress_fault(&self) -> bool {
        self.faults
            .iter()
            .any(|f| matches!(f, Fault::ProgressFault(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Initial state
    // =========================================================================

    #[test]
    fn new_monitor_is_not_all_off() {
        let m = FaultMonitor::new();
        assert!(!m.is_all_off());
    }

    #[test]
    fn new_monitor_has_no_alert() {
        let m = FaultMonitor::new();
        assert!(!m.alert_raised());
    }

    #[test]
    fn new_monitor_has_no_light_fault() {
        let m = FaultMonitor::new();
        assert!(!m.has_light_fault());
    }

    // =========================================================================
    // T-I7a | I7: Fault shutdown | SAF-06: Light fails to illuminate
    // Inject light fault (fail to illuminate) on any installation.
    // Expected: All installations go dark (allOff); alert is raised.
    // =========================================================================
    #[test]
    fn t_i7a_light_fault_illuminate_triggers_all_off() {
        let mut m = FaultMonitor::new();
        m.report(Fault::LightFailIlluminate(Direction::North));
        assert!(m.is_all_off());
        assert!(m.alert_raised());
    }

    // =========================================================================
    // T-I7b | I7: Fault shutdown | SAF-07: Light fails to de-illuminate
    // Inject light fault (fail to de-illuminate) on any installation.
    // Expected: All installations go dark (allOff); alert is raised.
    // =========================================================================
    #[test]
    fn t_i7b_light_fault_deilluminate_triggers_all_off() {
        let mut m = FaultMonitor::new();
        m.report(Fault::LightFailDeilluminate(Direction::East));
        assert!(m.is_all_off());
        assert!(m.alert_raised());
    }

    // =========================================================================
    // T-I7c | I7: Fault shutdown | SAF-05: Failure to progress
    // Inject progress fault on any installation.
    // Expected: All installations go dark (allOff); alert is raised.
    // =========================================================================
    #[test]
    fn t_i7c_progress_fault_triggers_all_off() {
        let mut m = FaultMonitor::new();
        m.report(Fault::ProgressFault(Direction::South));
        assert!(m.is_all_off());
        assert!(m.alert_raised());
    }

    // =========================================================================
    // Sensor fault does NOT trigger allOff — supports SAF-04 graceful degradation
    // =========================================================================
    #[test]
    fn sensor_fault_does_not_trigger_all_off() {
        let mut m = FaultMonitor::new();
        m.report(Fault::SensorFault(Direction::West));
        assert!(!m.is_all_off());
    }

    #[test]
    fn sensor_fault_raises_alert() {
        let mut m = FaultMonitor::new();
        m.report(Fault::SensorFault(Direction::West));
        assert!(m.alert_raised());
    }

    // =========================================================================
    // has_light_fault detection
    // =========================================================================
    #[test]
    fn has_light_fault_after_illuminate_fault() {
        let mut m = FaultMonitor::new();
        m.report(Fault::LightFailIlluminate(Direction::North));
        assert!(m.has_light_fault());
    }

    #[test]
    fn has_light_fault_after_deilluminate_fault() {
        let mut m = FaultMonitor::new();
        m.report(Fault::LightFailDeilluminate(Direction::North));
        assert!(m.has_light_fault());
    }

    #[test]
    fn no_light_fault_after_sensor_fault() {
        let mut m = FaultMonitor::new();
        m.report(Fault::SensorFault(Direction::North));
        assert!(!m.has_light_fault());
    }
}
