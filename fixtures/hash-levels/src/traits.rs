//! Behaviour of arena shapes: a trait with signatures and a default method, and its impls.
use std::fmt;

pub trait Shape: fmt::Debug {
    /// The covered area.
    fn area(&self) -> f32;
    fn perimeter(&self) -> f32;
    fn scaled(&self, k: f32) -> f32 { self.area() * k }
}

#[derive(Debug)]
pub struct Square { side: f32 }

impl Shape for Square {
    fn area(&self) -> f32 { self.side * self.side }
    fn perimeter(&self) -> f32 { 4.0 * self.side }
}

impl Square {
    pub fn new(side: f32) -> Self { Self { side: side.abs() } }
    pub fn grow(&mut self, by: f32) { self.side += by; log_growth(by); }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "square {}", self.side) }
}
