//! Banners hung from the line.
//! @implements KITE-BANNER@1

/// A banner's cloth.
pub struct Cloth {
    pub width: f32,
    pub height: f32,
}

/// The cloth's area, its hem trimmed.
pub fn cloth_area(cloth: &Cloth) -> f32 {
    let hem = 0.125;
    (cloth.width - hem) * (cloth.height - hem)
}
