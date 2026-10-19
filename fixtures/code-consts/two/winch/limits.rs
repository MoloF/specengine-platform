//! Winch limits: a file root; the last item does not parse.

// @assumes LD-0004
pub const WINCH_WEIGH: &str = "LD-0004";

pub const WINCH_BRAKE: &str = brake_code();

// @implements LD-0004
pub const WINCH_FAULT: &str = { let code = 1 2; "LD-0004" };
