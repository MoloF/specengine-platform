//! Gust peak: moved across files and edited.

/// The mean wind's share of a peak.
pub fn mean_share(wind: f32) -> f32 {
    wind * 0.5
}

// @implements KITE-STRAY@1
pub fn gust_peak(mean: f32, spread: f32, seconds: f32) -> f32 {
    let wave = (seconds * 0.5).sin() * spread;
    let crest = mean + wave.abs() * 1.25;
    let capped = crest.min(mean * 3.0 + spread);
    if capped < mean { mean } else { capped }
}
