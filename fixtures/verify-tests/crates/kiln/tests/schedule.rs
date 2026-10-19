//! @verifies KILN-HOLD
//! The firing schedule end to end: the ramp meets the hold.

use kiln_core::ramp::next_temperature;

// @verifies KILN-RAMP
// @verifies KILN-HOLD
#[test]
fn hold_follows_the_ramp() {
    assert_eq!(next_temperature(1150, 100, 1200), 1200);
}

// @verifies glaze-check/GLZ-01
#[test]
fn glaze_survives_the_hold() {
    assert_eq!(next_temperature(1200, 0, 1200), 1200);
}
