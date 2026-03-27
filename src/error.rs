use std::fmt;

/// Errors returned when a signal transition is rejected due to a violated
/// precondition (from the Hoare triples in the formal specification).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionError {
    /// The system is in the allOff fault state. No transitions permitted (I8).
    SystemShutdown,
    /// Pedestrians are currently crossing. No transitions permitted (I2, SAF-02).
    PedestrianCrossing,
    /// The intersecting pair is active. Transition would violate mutual exclusion (I1, SAF-01).
    ConflictingSignal,
    /// A light fault has been detected. System must shut down (I7, SAF-06/07).
    LightFault,
    /// The signal has not progressed in time. System must shut down (I7, SAF-05).
    ProgressFault,
    /// The transition is not valid for the current state.
    InvalidTransition,
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unimplemented!()
    }
}

impl std::error::Error for TransitionError {}
