use std::time::{Duration, Instant};

/// Abstraction over time, allowing tests to control the clock.
pub trait Clock {
    /// Returns the current instant.
    fn now(&self) -> Instant;
}

/// Uses the real system clock.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// A fake clock for testing. Time only advances when you call `advance()`.
pub struct MockClock {
    // TODO: implement using std::cell::Cell<Instant> for interior mutability
    //   current: std::cell::Cell<Instant>,
}

impl MockClock {
    /// Creates a new mock clock starting at the current instant.
    pub fn new() -> Self {
        unimplemented!()
    }

    /// Advances time forward by the given duration.
    pub fn advance(&self, duration: Duration) {
        unimplemented!()
    }
}

impl Clock for MockClock {
    fn now(&self) -> Instant {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_clock_advance_changes_now() {
        let clock = MockClock::new();
        let t0 = clock.now();
        clock.advance(Duration::from_secs(5));
        let t1 = clock.now();
        assert_eq!(t1 - t0, Duration::from_secs(5));
    }

    #[test]
    fn mock_clock_advance_accumulates() {
        let clock = MockClock::new();
        let t0 = clock.now();
        clock.advance(Duration::from_millis(1500));
        clock.advance(Duration::from_millis(1500));
        let t1 = clock.now();
        assert_eq!(t1 - t0, Duration::from_secs(3));
    }
}
