//! Lift codes of the crane panel.

// @implements CR-0001@1
pub const HOOK_SPEED: &str = "CR-0001";

pub const WIND_STOP: &str = r"CR-0002";

pub const JIB_SWING: &str = "JB-0005";

pub const PARKED: &str = "TS-0001";

pub const GUST: &str = "W17";

pub const OVERLOAD: &str = "LOAD12";

pub const WEIGH_FIRST: &str = "LD-0004";

pub const HORN: &str = "horn";

pub const SPOTTER: &str = "LD-000";

pub const SPEED_TEXT: String = String::new();

pub const NIGHT: &str = include_str!("night.txt");

pub const LIMIT_TAB: &str = "LD-\u{30}006";

pub const HOOK_BYTES: &CStr = c"CR-0001";

pub const FIRST_LETTER: char = 'C';

pub const ALL_LIFTS: &[&str] = &["CR-0001", "CR-0002"];

// @verifies LD-0006
pub const SHORT_HOLD: &str = ("LD-0006");

pub const PENDING: &str;

pub const SAME_AS_HOOK: &str = HOOK_SPEED;

pub const STORM_NOTE: &str = r"TS-0001
parks every crane";

pub static QUAY_LIMIT: &str = "LD-0006";

mod night {
    #[cfg(test)]
    pub const LIGHTS: &str = "CR-0007";

    #[cfg(not(test))]
    pub const LIGHTS: &str = "CR-0007";
}

pub enum Signal {
    Stop,
    Hoist,
}

trait Lift {
    const CODE: &'static str = "CR-0001";
}

fn wind_code() -> &'static str {
    static INNER: &str = "CR-0001";
    INNER
}
