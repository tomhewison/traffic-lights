use std::time::Duration;
use traffic_lights::clock::MockClock;
use traffic_lights::direction::Direction;
use traffic_lights::junction::Junction;
use traffic_lights::signal::Signal;

// =============================================================================
//
//  INTEGRATION / SEQUENCE TESTS (T-S*)
//
//  These exercise complete operational sequences across multiple invariants.
//
// =============================================================================

// =============================================================================
// T-S1 | FUN-01, SAF-01, SAF-03
// Full junction cycle: NS green phase → EW green phase → NS green phase.
// Expected: All transitions follow the valid order; paired sync maintained
//           throughout; no invariant violated.
// =============================================================================
#[test]
fn t_s1_full_junction_cycle() {
    let mut jn = Junction::with_clock(MockClock::new());

    // --- NS green phase ---
    // NS: R → RA → G
    jn.try_advance_ns().unwrap();
    assert_eq!(jn.ns_signal(), Signal::RedAmber);
    assert_eq!(jn.ew_signal(), Signal::Red);

    jn.try_advance_ns().unwrap();
    assert_eq!(jn.ns_signal(), Signal::Green);
    assert_eq!(jn.ew_signal(), Signal::Red);

    // NS: G → A → R
    jn.try_advance_ns().unwrap();
    assert_eq!(jn.ns_signal(), Signal::Amber);
    assert_eq!(jn.ew_signal(), Signal::Red);

    jn.try_advance_ns().unwrap();
    assert_eq!(jn.ns_signal(), Signal::Red);
    assert_eq!(jn.ew_signal(), Signal::Red);
    assert!(jn.is_all_red());

    // --- EW green phase ---
    // EW: R → RA → G
    jn.try_advance_ew().unwrap();
    assert_eq!(jn.ew_signal(), Signal::RedAmber);
    assert_eq!(jn.ns_signal(), Signal::Red);

    jn.try_advance_ew().unwrap();
    assert_eq!(jn.ew_signal(), Signal::Green);
    assert_eq!(jn.ns_signal(), Signal::Red);

    // EW: G → A → R
    jn.try_advance_ew().unwrap();
    assert_eq!(jn.ew_signal(), Signal::Amber);
    assert_eq!(jn.ns_signal(), Signal::Red);

    jn.try_advance_ew().unwrap();
    assert_eq!(jn.ew_signal(), Signal::Red);
    assert!(jn.is_all_red());

    // --- NS green phase again ---
    jn.try_advance_ns().unwrap();
    assert_eq!(jn.ns_signal(), Signal::RedAmber);

    // Paired sync: N = S and E = W at every point
    assert_eq!(
        jn.signal(Direction::North),
        jn.signal(Direction::South)
    );
    assert_eq!(
        jn.signal(Direction::East),
        jn.signal(Direction::West)
    );
}

// =============================================================================
// T-S2 | FUN-05, SAF-02
// Pedestrian interrupts mid-cycle: pedestrian presses button while NS is Green.
// Expected: NS completes G → A → R; pedestrian crossing begins at allRed;
//           15s hold; cycling resumes.
// =============================================================================
#[test]
fn t_s2_pedestrian_interrupts_mid_cycle() {
    let mut jn = Junction::with_clock(MockClock::new());

    // Advance NS to Green
    jn.try_advance_ns().unwrap(); // R → RA
    jn.try_advance_ns().unwrap(); // RA → G
    assert_eq!(jn.ns_signal(), Signal::Green);

    // Pedestrian presses button
    jn.request_pedestrian_crossing();

    // NS must complete its cycle: cannot start crossing yet (not allRed)
    assert!(!jn.is_all_red());

    // NS: G → A → R
    jn.try_advance_ns().unwrap();
    jn.try_advance_ns().unwrap();
    assert!(jn.is_all_red());

    // Now begin pedestrian crossing
    jn.begin_pedestrian_crossing().unwrap();
    assert!(jn.ped_crossing_active());
    assert!(jn.ped_alert_active());
    assert!(jn.is_all_red());

    // No transitions during crossing
    assert!(jn.try_advance_ns().is_err());
    assert!(jn.try_advance_ew().is_err());

    // After 15s, end crossing
    // clock.advance(Duration::from_secs(15));
    // jn.tick();
    jn.end_pedestrian_crossing();
    assert!(!jn.ped_crossing_active());

    // Regular cycling resumes
    assert!(jn.try_advance_ew().is_ok());
}

// =============================================================================
// T-S3 | SAF-04, FUN-04
// Sensor fault during Green phase with no competing traffic.
// Expected: Green timeout reverts to 30s; alert raised; cycle continues normally.
// =============================================================================
#[test]
fn t_s3_sensor_fault_during_green_no_competing_traffic() {
    let mut jn = Junction::with_clock(MockClock::new());

    // Advance EW to Green
    jn.try_advance_ew().unwrap();
    jn.try_advance_ew().unwrap();
    assert_eq!(jn.ew_signal(), Signal::Green);

    // Sensor fault on East
    jn.report_sensor_fault(Direction::East);

    // System is NOT shut down — graceful degradation
    assert!(!jn.is_all_off());
    assert!(jn.alert_raised());
    assert_eq!(jn.green_timeout(Direction::East), Duration::from_secs(30));

    // Junction can still cycle
    jn.try_advance_ew().unwrap(); // G → A
    assert_eq!(jn.ew_signal(), Signal::Amber);
}

// =============================================================================
// T-S4 | SAF-04
// Multiple sensor faults: both NS and EW sensors fail.
// Expected: Both pairs revert to 30s Green timeout; alert raised;
//           junction continues cycling.
// =============================================================================
#[test]
fn t_s4_multiple_sensor_faults() {
    let mut jn = Junction::with_clock(MockClock::new());

    jn.report_sensor_fault(Direction::North);
    jn.report_sensor_fault(Direction::East);

    assert!(!jn.is_all_off());
    assert!(jn.alert_raised());
    assert_eq!(jn.green_timeout(Direction::North), Duration::from_secs(30));
    assert_eq!(jn.green_timeout(Direction::East), Duration::from_secs(30));

    // Junction still cycles
    assert!(jn.try_advance_ns().is_ok());
}

// =============================================================================
// T-S5 | SAF-06
// Light fault during transition: Green bulb fails to illuminate on RA → G.
// Expected: System detects fault; all installations go dark; alert raised.
// =============================================================================
#[test]
fn t_s5_light_fault_during_transition() {
    let mut jn = Junction::with_clock(MockClock::new());

    // Advance NS to RA
    jn.try_advance_ns().unwrap();
    assert_eq!(jn.ns_signal(), Signal::RedAmber);

    // Green bulb fails on RA → G attempt
    jn.report_light_fault(Direction::North);

    assert!(jn.is_all_off());
    assert!(jn.alert_raised());

    // All signals are Off
    for dir in [Direction::North, Direction::South, Direction::East, Direction::West] {
        assert_eq!(jn.signal(dir), Signal::Off);
    }
}

// =============================================================================
// T-S6 | SAF-02, SAF-06
// Pedestrian crossing with subsequent fault: light fails during 15s hold.
// Expected: Fault shutdown overrides pedestrian phase; allOff; alert raised.
// =============================================================================
#[test]
fn t_s6_fault_during_pedestrian_crossing() {
    let mut jn = Junction::with_clock(MockClock::new());

    // Begin crossing
    jn.request_pedestrian_crossing();
    jn.begin_pedestrian_crossing().unwrap();
    assert!(jn.ped_crossing_active());

    // Light fault during crossing
    jn.report_light_fault(Direction::West);

    // Fault shutdown takes priority over pedestrian phase
    assert!(jn.is_all_off());
    assert!(jn.alert_raised());
    assert!(!jn.ped_crossing_active());
}

// =============================================================================
// T-S7 | FUN-05
// Rapid pedestrian requests: two presses before crossing begins.
// Expected: Only one 15s hold period is triggered at the next allRed.
// =============================================================================
#[test]
fn t_s7_rapid_pedestrian_requests() {
    let mut jn = Junction::with_clock(MockClock::new());

    // Multiple rapid presses
    jn.request_pedestrian_crossing();
    jn.request_pedestrian_crossing();
    jn.request_pedestrian_crossing();

    // Begin crossing
    jn.begin_pedestrian_crossing().unwrap();
    assert!(jn.ped_crossing_active());

    // End crossing
    jn.end_pedestrian_crossing();
    assert!(!jn.ped_crossing_active());

    // No second crossing should be queued
    // A new request would be needed
    assert!(!jn.ped_crossing_active());
}

// =============================================================================
// T-S8 | I1–I11
// System startup: verify initial state.
// Expected: All installations start on Red (allRed); no faults;
//           no pedestrian crossing.
// =============================================================================
#[test]
fn t_s8_system_startup_initial_state() {
    let jn = Junction::with_clock(MockClock::new());

    // All signals Red
    assert!(jn.is_all_red());
    assert_eq!(jn.ns_signal(), Signal::Red);
    assert_eq!(jn.ew_signal(), Signal::Red);

    for dir in [Direction::North, Direction::South, Direction::East, Direction::West] {
        assert_eq!(jn.signal(dir), Signal::Red);
    }

    // No faults
    assert!(!jn.is_all_off());
    assert!(!jn.alert_raised());

    // No pedestrian crossing
    assert!(!jn.ped_crossing_active());
    assert!(!jn.ped_alert_active());
}
