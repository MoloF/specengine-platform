//! Trailing commas in every position rustfmt touches: struct literals, call
//! arguments, arrays, match arms, parameter lists and generic bounds. The
//! contrasting config (`max_width = 60`) reflows the long lines and adds or
//! removes commas; the default config removes the one-line trailing commas.

#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub x: i64,
    pub y: i64,
}

#[derive(Debug)]
pub enum Shape {
    Dot,
    Line(Point, Point),
    Triangle { a: Point, b: Point, c: Point },
}

pub fn origin() -> Point {
    Point { x: 0, y: 0 }
}

pub fn shifted(point: Point, dx: i64, dy: i64) -> Point {
    Point { x: point.x + dx, y: point.y + dy, }
}

pub fn corners() -> [Point; 3] {
    [Point { x: 0, y: 0 }, Point { x: 1, y: 0 }, Point { x: 0, y: 1 },]
}

pub fn side_count(shape: &Shape) -> usize {
    match shape {
        Shape::Dot => 0,
        Shape::Line(_, _) => 1,
        Shape::Triangle { .. } => 3,
    }
}

pub fn combine_three_values(first: i64, second: i64, third: i64) -> i64 {
    first * 100 + second * 10 + third
}

pub fn describe_everything(first_argument: i64, second_argument: i64, third_one: i64) -> i64 {
    combine_three_values(first_argument, second_argument, third_one)
}

pub fn longest_call_site() -> i64 {
    combine_three_values(1_000_000_007, 2_000_000_011, 3_000_000_019) + describe_everything(1, 2, 3)
}

pub fn generic_pair<First: Clone + Default, Second: Clone + Default>(first: First) -> (First, Second) {
    (first, Second::default())
}
