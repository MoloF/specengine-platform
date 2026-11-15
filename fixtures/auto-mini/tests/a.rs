//! The door timer's one test.

#[test]
fn a_door_closes_after_five_seconds() {
    assert!(auto_mini::door_open(4));
    assert!(!auto_mini::door_open(5));
}
