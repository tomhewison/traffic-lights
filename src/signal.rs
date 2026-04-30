use crate::signal::Signal::{Amber, Green, Off, Red, RedAmber};

/// Represents the signal state of a single traffic light installation.
///
/// The valid transition cycle is: Red → RedAmber → Green → Amber → Red.
/// Off is a terminal fault state — no transitions are permitted from Off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Signal {
    Red,
    RedAmber,
    Green,
    Amber,
    Off,
}

impl Signal {
    /// Returns the next valid signal in the cycle.
    /// Off is terminal and maps to Off.
    pub fn next(self) -> Signal {
        // match feels very nice in rust
        match self {
            Red => RedAmber,
            RedAmber => Green,
            Green => Amber,
            Amber => Red,
            Off => {
                // This maps to off as my thinking is manual intervention is most likely needed. (I8)
                Off
            }
        }
    }

    /// Returns true if the signal permits or implies traffic movement.
    /// RedAmber (transition into movement), Green, and Amber (clearing) are active.
    /// Red and Off are not active.
    pub fn is_active(self) -> bool {
        match self {
            RedAmber | Green | Amber => true,
            Red | Off => false,
        }
    }

    /// Returns the maximum duration this signal phase should be held,
    /// or None if the duration is context-dependent (e.g. Green depends on traffic).
    pub fn max_duration(self) -> Option<std::time::Duration> {
        match self {
            Green | Red | Off => None,
            RedAmber | Amber => Some(std::time::Duration::from_millis(1500)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // T-I4c | I4: Valid transition order | FUN-01: Signal transition order
    // Walk through the full cycle R → RA → G → A → R.
    // Expected: Each transition succeeds in order; final state is Red.
    // =========================================================================
    #[test]
    fn t_i4c_full_cycle_r_ra_g_a_r() {
        let mut s = Red;

        s = s.next();
        assert_eq!(s, RedAmber);

        s = s.next();
        assert_eq!(s, Green);

        s = s.next();
        assert_eq!(s, Amber);

        s = s.next();
        assert_eq!(s, Red);
    }

    // =========================================================================
    // T-I4a | I4: Valid transition order | FUN-01: Signal transition order
    // Attempt to transition from Red directly to Green (skipping Red+Amber).
    // Expected: Transition is rejected; Red.next() must be RedAmber, not Green.
    // =========================================================================
    #[test]
    fn t_i4a_red_cannot_skip_to_green() {
        let s = Red;
        assert_eq!(s.next(), RedAmber);
        assert_ne!(s.next(), Green);
    }

    // =========================================================================
    // T-I4b | I4: Valid transition order | FUN-01: Signal transition order
    // Attempt to transition from Green directly to Red (skipping Amber).
    // Expected: Transition is rejected; Green.next() must be Amber, not Red.
    // =========================================================================
    #[test]
    fn t_i4b_green_cannot_skip_to_red() {
        let s = Green;
        assert_eq!(s.next(), Amber);
        assert_ne!(s.next(), Red);
    }

    // =========================================================================
    // Additional signal unit tests — Off state is terminal
    // I8: Fault state is terminal
    // =========================================================================
    #[test]
    fn off_is_terminal() {
        assert_eq!(Off.next(), Off);
    }

    // =========================================================================
    // is_active correctness — supports I1 mutual exclusion checks
    // =========================================================================
    #[test]
    fn red_is_not_active() {
        assert!(!Red.is_active());
    }

    #[test]
    fn red_amber_is_active() {
        assert!(RedAmber.is_active());
    }

    #[test]
    fn green_is_active() {
        assert!(Green.is_active());
    }

    #[test]
    fn amber_is_active() {
        assert!(Amber.is_active());
    }

    #[test]
    fn off_is_not_active() {
        assert!(!Off.is_active());
    }

    // =========================================================================
    // max_duration — supports I9 (intermediate timing)
    // FUN-02: Red+Amber and Amber are 1.5s each
    // =========================================================================
    #[test]
    fn red_amber_max_duration_is_1500ms() {
        assert_eq!(
            RedAmber.max_duration(),
            Some(std::time::Duration::from_millis(1500))
        );
    }

    #[test]
    fn amber_max_duration_is_1500ms() {
        assert_eq!(
            Amber.max_duration(),
            Some(std::time::Duration::from_millis(1500))
        );
    }

    #[test]
    fn green_max_duration_is_none() {
        // Green duration is context-dependent (30s or infinite)
        assert_eq!(Green.max_duration(), None);
    }

    #[test]
    fn red_max_duration_is_none() {
        assert_eq!(Red.max_duration(), None);
    }
}
