//! @verifies PIECE-COUNTER
use tally_kit::Tally;

// @verifies LAW-BOUND@2 [body] mutation: drop the cap
#[test]
fn stops_at_cap() {
    let mut tally = Tally::default();
    for _ in 0..11 {
        tally.step();
    }
    assert_eq!(tally.value(), 10);
}

// @verifies cap-check/PASS-01 [Sig]
#[test]
fn steps_by_one() {
    let mut tally = Tally::default();
    tally.step();
    assert_eq!(tally.value(), 1);
}
