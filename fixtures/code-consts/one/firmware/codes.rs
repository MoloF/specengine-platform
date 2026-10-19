//! Rule codes of the irrigation controller.

// @implements VLV-001@1
// @verifies VLV-002
pub const VALVE_LOW_PRESSURE: &str = "VLV-001";

pub const VALVE_WINDOW: &str = r#"VLV-002"#;

pub const AFTER_RAIN: &str = "WTR-007";

pub const TWICE_A_DAY: &str = "IRR-003";

pub const MIST_FLOOR: &str = "ACC-001";

pub const HUMIDITY_HIGH: &str = "H204";

pub const HUMIDITY_SENSOR: &str = "H2041";

pub const DAWN_START: &str = "IRR-005";

pub const PUMP_IDLE: &str = "pump idle";

pub const ESCAPED: &str = "VLV-\x30\x30\x31";

pub const RAW_BYTES: &[u8] = b"VLV-001";

// @implements VLV-002@2
pub const JOINED: &str = concat!("VLV", "-002");

pub const RETRIES: u32 = 3;

pub static LAST_VALVE: &str = "VLV-006";

pub mod frost {
    pub const FROST_DELAY: &str = "IRR-009";
}

#[cfg(feature = "north")]
pub const ZONE_COUNT: &str = "VLV-004";

#[cfg(not(feature = "north"))]
pub const ZONE_COUNT: &str = "VLV-004";

pub const NOTICE: &str = "VLV-001
closes the line";

pub enum Alarm {
    Frost,
    Rain,
}

impl Alarm {
    pub const FROST_CODE: &'static str = "VLV-002";
}

pub fn pressure_code() -> &'static str {
    const INNER: &str = "VLV-001";
    INNER
}
