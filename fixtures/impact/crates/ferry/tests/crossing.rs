//! The crossing checks.

use ferry::crossing::Crossing;

// @verifies ferry-check/CHK-01
#[test]
fn waits_for_the_window() {
    let crossing = Crossing { open: 6, close: 9 };
    assert_eq!(crossing.opens(), 6);
}

// @verifies ferry-check/CHK-02
#[test]
fn slack_widens_the_window() {
    assert_eq!(ferry::crossing::slack::margin(), 5);
}

// @verifies TIDE-WINDOW
#[test]
fn window_closes() {
    let crossing = Crossing { open: 6, close: 9 };
    assert_eq!(crossing.closes(), 9);
}
