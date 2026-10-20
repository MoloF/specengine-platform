//! The flight planner: a binary until a library joins it.

fn main() {
    let route = plan_route(3.0, 4.0);
    let rest = plan_rest(route, 2.0);
    println!("{route} {rest}");
}

// @implements KITE-TOOL@1
fn plan_route(east: f32, north: f32) -> f32 {
    let leg = (east * east + north * north).sqrt();
    let wind = leg * 0.125 + 0.5;
    let calm = wind.min(leg) * 0.25;
    let total = leg + wind * 2.0 - calm;
    if total > 100.0 { 100.0 } else { total }
}

// @implements KITE-TOOL@1
fn plan_rest(route: f32, pace: f32) -> f32 {
    let hours = route / (pace + 0.25);
    let stops = (hours / 3.0).floor() + 1.0;
    let tired = (hours * 0.5).min(stops);
    let rest = stops * 0.75 - hours * 0.0625 + tired;
    if rest < 0.5 { 0.5 } else { rest }
}
