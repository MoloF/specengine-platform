//! Cooling: the kiln cools by its rate and stops at zero.

// @implements KILN-COOL@1
pub fn cool_step(now: u32, rate: u32) -> u32 {
    now.saturating_sub(rate)
}

#[cfg(test)]
mod tests {
    use super::cool_step;

    // @verifies KILN-COOL@2
    #[test]
    fn cooling_stops_at_zero() {
        assert_eq!(cool_step(10, 50), 0);
    }
}
