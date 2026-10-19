//! The ramp: degrees per hour up to the peak.

// @implements KILN-RAMP
pub fn next_temperature(now: u32, rate: u32, peak: u32) -> u32 {
    (now + rate).min(peak)
}

#[cfg(test)]
mod tests {
    use super::next_temperature;

    // @verifies KILN-RAMP
    #[test]
    fn ramp_stops_at_peak() {
        assert_eq!(next_temperature(1190, 50, 1200), 1200);
    }

    // @verifies CONE-01
    #[cfg(unix)]
    #[test]
    fn peak_is_capped() {
        assert!(next_temperature(0, 5000, 1300) <= 1300);
    }

    // @verifies PYRO-01
    #[cfg(not(unix))]
    #[test]
    fn peak_is_capped() {
        assert!(next_temperature(0, 5000, 1300) <= 1300);
    }
}
