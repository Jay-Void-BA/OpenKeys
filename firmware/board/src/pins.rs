// Generated from the frozen R04 pinout.csv; do not infer a different pinmux.
// Board SHA256: 6c2a93a63f343cf0401eae8c92dc1da6f5db145604ee9b0e8b9bd08b76f4bde4
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pin {
    pub package_pin: u8,
    pub port: u8,
    pub bit: u8,
    pub alternate: Option<u8>,
}
pub const BOARD_REVISION: &str = "0.4";
pub const BOARD_SHA256: &str = "6c2a93a63f343cf0401eae8c92dc1da6f5db145604ee9b0e8b9bd08b76f4bde4";
pub const PINOUT_SHA256: &str = "015f236a2066e38bd63a31aea3ea134474402bfe2becfdf4af9b7557d0dacac5";
pub const CASE_CLOSED: Pin = Pin {
    package_pin: 2,
    port: b'C',
    bit: 13,
    alternate: None,
};
pub const LSE_IN: Pin = Pin {
    package_pin: 3,
    port: b'C',
    bit: 14,
    alternate: None,
};
pub const LSE_OUT: Pin = Pin {
    package_pin: 4,
    port: b'C',
    bit: 15,
    alternate: None,
};
pub const USB_GUARD_FAULT_N: Pin = Pin {
    package_pin: 8,
    port: b'C',
    bit: 0,
    alternate: None,
};
pub const NRST: Pin = Pin {
    package_pin: 9,
    port: b'C',
    bit: 1,
    alternate: None,
};
pub const FLASH_MISO: Pin = Pin {
    package_pin: 10,
    port: b'C',
    bit: 2,
    alternate: Some(5),
};
pub const LCD_RESET_N: Pin = Pin {
    package_pin: 11,
    port: b'C',
    bit: 3,
    alternate: None,
};
pub const CHG_STAT1_N: Pin = Pin {
    package_pin: 16,
    port: b'A',
    bit: 2,
    alternate: None,
};
pub const CHG_STAT2_N: Pin = Pin {
    package_pin: 17,
    port: b'A',
    bit: 3,
    alternate: None,
};
pub const NFC_CS_N: Pin = Pin {
    package_pin: 20,
    port: b'A',
    bit: 4,
    alternate: None,
};
pub const NFC_SCK: Pin = Pin {
    package_pin: 21,
    port: b'A',
    bit: 5,
    alternate: Some(5),
};
pub const NFC_MISO: Pin = Pin {
    package_pin: 22,
    port: b'A',
    bit: 6,
    alternate: Some(5),
};
pub const NFC_MOSI: Pin = Pin {
    package_pin: 23,
    port: b'A',
    bit: 7,
    alternate: Some(5),
};
pub const LCD_BACKLIGHT: Pin = Pin {
    package_pin: 24,
    port: b'C',
    bit: 4,
    alternate: None,
};
pub const VBUS_PRESENT_N: Pin = Pin {
    package_pin: 25,
    port: b'C',
    bit: 5,
    alternate: None,
};
pub const BTN1_N: Pin = Pin {
    package_pin: 26,
    port: b'B',
    bit: 0,
    alternate: None,
};
pub const BTN2_N: Pin = Pin {
    package_pin: 27,
    port: b'B',
    bit: 1,
    alternate: None,
};
pub const BTN3_N: Pin = Pin {
    package_pin: 28,
    port: b'B',
    bit: 2,
    alternate: None,
};
pub const NFC_IRQ_N: Pin = Pin {
    package_pin: 29,
    port: b'B',
    bit: 10,
    alternate: None,
};
pub const FLASH_CS_N: Pin = Pin {
    package_pin: 33,
    port: b'B',
    bit: 12,
    alternate: None,
};
pub const FLASH_SCK: Pin = Pin {
    package_pin: 34,
    port: b'B',
    bit: 13,
    alternate: Some(5),
};
pub const FLASH_MOSI: Pin = Pin {
    package_pin: 36,
    port: b'B',
    bit: 15,
    alternate: Some(5),
};
pub const FP_IRQ: Pin = Pin {
    package_pin: 37,
    port: b'C',
    bit: 6,
    alternate: None,
};
pub const FP_RESET_N: Pin = Pin {
    package_pin: 38,
    port: b'C',
    bit: 7,
    alternate: None,
};
pub const SE_ENABLE_REQUEST: Pin = Pin {
    package_pin: 39,
    port: b'C',
    bit: 8,
    alternate: None,
};
pub const LCD_SDA: Pin = Pin {
    package_pin: 40,
    port: b'C',
    bit: 9,
    alternate: Some(4),
};
pub const LCD_SCL: Pin = Pin {
    package_pin: 41,
    port: b'A',
    bit: 8,
    alternate: Some(4),
};
pub const USB_DM: Pin = Pin {
    package_pin: 44,
    port: b'A',
    bit: 11,
    alternate: Some(10),
};
pub const USB_DP: Pin = Pin {
    package_pin: 45,
    port: b'A',
    bit: 12,
    alternate: Some(10),
};
pub const SWDIO: Pin = Pin {
    package_pin: 46,
    port: b'A',
    bit: 13,
    alternate: None,
};
pub const SWCLK: Pin = Pin {
    package_pin: 49,
    port: b'A',
    bit: 14,
    alternate: None,
};
pub const FP_CS_N: Pin = Pin {
    package_pin: 50,
    port: b'A',
    bit: 15,
    alternate: None,
};
pub const FP_SCK: Pin = Pin {
    package_pin: 51,
    port: b'C',
    bit: 10,
    alternate: Some(6),
};
pub const FP_MISO: Pin = Pin {
    package_pin: 52,
    port: b'C',
    bit: 11,
    alternate: Some(6),
};
pub const FP_MOSI: Pin = Pin {
    package_pin: 53,
    port: b'C',
    bit: 12,
    alternate: Some(6),
};
pub const SWO: Pin = Pin {
    package_pin: 55,
    port: b'B',
    bit: 3,
    alternate: None,
};
pub const CHARGE_ENABLE: Pin = Pin {
    package_pin: 56,
    port: b'B',
    bit: 4,
    alternate: None,
};
pub const BTN4_N: Pin = Pin {
    package_pin: 57,
    port: b'B',
    bit: 5,
    alternate: None,
};
pub const SE_SCL: Pin = Pin {
    package_pin: 58,
    port: b'B',
    bit: 6,
    alternate: Some(4),
};
pub const SE_SDA: Pin = Pin {
    package_pin: 59,
    port: b'B',
    bit: 7,
    alternate: Some(4),
};
pub const PIN_MAP: &[(&str, Pin)] = &[
    ("CASE_CLOSED", CASE_CLOSED),
    ("LSE_IN", LSE_IN),
    ("LSE_OUT", LSE_OUT),
    ("USB_GUARD_FAULT_N", USB_GUARD_FAULT_N),
    ("NRST", NRST),
    ("FLASH_MISO", FLASH_MISO),
    ("LCD_RESET_N", LCD_RESET_N),
    ("CHG_STAT1_N", CHG_STAT1_N),
    ("CHG_STAT2_N", CHG_STAT2_N),
    ("NFC_CS_N", NFC_CS_N),
    ("NFC_SCK", NFC_SCK),
    ("NFC_MISO", NFC_MISO),
    ("NFC_MOSI", NFC_MOSI),
    ("LCD_BACKLIGHT", LCD_BACKLIGHT),
    ("VBUS_PRESENT_N", VBUS_PRESENT_N),
    ("BTN1_N", BTN1_N),
    ("BTN2_N", BTN2_N),
    ("BTN3_N", BTN3_N),
    ("NFC_IRQ_N", NFC_IRQ_N),
    ("FLASH_CS_N", FLASH_CS_N),
    ("FLASH_SCK", FLASH_SCK),
    ("FLASH_MOSI", FLASH_MOSI),
    ("FP_IRQ", FP_IRQ),
    ("FP_RESET_N", FP_RESET_N),
    ("SE_ENABLE_REQUEST", SE_ENABLE_REQUEST),
    ("LCD_SDA", LCD_SDA),
    ("LCD_SCL", LCD_SCL),
    ("USB_DM", USB_DM),
    ("USB_DP", USB_DP),
    ("SWDIO", SWDIO),
    ("SWCLK", SWCLK),
    ("FP_CS_N", FP_CS_N),
    ("FP_SCK", FP_SCK),
    ("FP_MISO", FP_MISO),
    ("FP_MOSI", FP_MOSI),
    ("SWO", SWO),
    ("CHARGE_ENABLE", CHARGE_ENABLE),
    ("BTN4_N", BTN4_N),
    ("SE_SCL", SE_SCL),
    ("SE_SDA", SE_SDA),
];
