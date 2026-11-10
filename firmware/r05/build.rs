use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = here.join("../..");
    let contract = here.join("board-contract.sha256");
    println!("cargo:rerun-if-changed={}", contract.display());
    let contract = fs::read_to_string(contract).expect("R05 reviewed board contract is missing");
    assert_eq!(
        contract.lines().count(),
        2,
        "Expected PCB and pinout bindings"
    );
    let names: Vec<_> = contract
        .lines()
        .map(|row| row.split_once("  ").unwrap().1)
        .collect();
    assert_eq!(
        names,
        [
            "hardware/r05/kicad/OpenKeys.kicad_pcb",
            "hardware/r05/pinout.csv"
        ]
    );
    for row in contract.lines() {
        let (expected, name) = row.split_once("  ").expect("Invalid contract row");
        assert!(name.starts_with("hardware/r05/"));
        assert!(!name.contains(".."));
        let path = root.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        let actual = format!("{:x}", Sha256::digest(fs::read(path).unwrap()));
        assert_eq!(
            actual, expected,
            "R05 hardware changed; review and freeze a new contract"
        );
    }
    let csv = fs::read_to_string(root.join("hardware/r05/pinout.csv")).unwrap();
    let rows: Vec<Vec<&str>> = csv
        .lines()
        .skip(1)
        .map(|r| r.split(',').collect())
        .collect();
    // Independent required-function list. Source code below uses these same
    // typed HAL pins; the CSV is generated from the actual MCU schematic.
    for (signal, gpio, pin, function) in [
        ("BTN1_N", "PB0", "26", "power / GPIO / dedicated"),
        ("BTN2_N", "PB1", "27", "power / GPIO / dedicated"),
        ("BTN3_N", "PB2", "28", "power / GPIO / dedicated"),
        ("BTN4_N", "PB5", "57", "power / GPIO / dedicated"),
        ("BTN5_N", "PC6", "37", "power / GPIO / dedicated"),
        ("BTN6_N", "PC7", "38", "power / GPIO / dedicated"),
        ("BTN7_N", "PA15", "50", "power / GPIO / dedicated"),
        ("BTN8_N", "PC10", "51", "power / GPIO / dedicated"),
        ("BTN9_N", "PC11", "52", "power / GPIO / dedicated"),
        ("BTN0_N", "PC12", "53", "power / GPIO / dedicated"),
        ("LCD_RELEASE", "PC3", "11", "power / GPIO / dedicated"),
        ("LCD_ARM_CLK", "PC4", "24", "power / GPIO / dedicated"),
        ("LCD_HOST_SCL", "PA8", "41", "I2C3_SCL AF4"),
        ("LCD_HOST_SDA", "PC9", "40", "I2C3_SDA AF4"),
        ("SE_ENABLE_REQUEST", "PC8", "39", "power / GPIO / dedicated"),
        ("CHARGE_ENABLE", "PB4", "56", "power / GPIO / dedicated"),
        ("CASE_CLOSED", "PC13", "2", "TAMP_IN1 dedicated/VBAT"),
        ("USB_GUARD_FAULT_N", "PC0", "8", "power / GPIO / dedicated"),
        ("FLASH_CS_N", "PB12", "33", "power / GPIO / dedicated"),
        ("FLASH_SCK", "PB13", "34", "SPI2_SCK AF5"),
        ("FLASH_MOSI", "PB15", "36", "SPI2_MOSI AF5"),
        ("FLASH_MISO", "PC2", "10", "SPI2_MISO AF5"),
        ("NFC_CS_N", "PA4", "20", "power / GPIO / dedicated"),
    ] {
        let matching: Vec<_> = rows.iter().filter(|r| r[2] == signal).collect();
        assert_eq!(
            matching.len(),
            1,
            "Missing or duplicate R05 signal: {signal}"
        );
        assert_eq!(matching[0][0], pin);
        assert_eq!(matching[0][1], gpio);
        assert_eq!(matching[0][3], function);
    }
    assert!(!csv.contains("FP_") && !csv.contains("LCD_RESET_N"));
    if env::var_os("CARGO_FEATURE_R05_DIAGNOSTICS").is_none() {
        return;
    }
    assert_eq!(env::var("TARGET").unwrap(), "thumbv8m.main-none-eabihf");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    // Bring-up ONLY. Never use this OPEN/TZEN=0 map for owner secrets.
    fs::write(output.join("memory.x"), "MEMORY {\n FLASH : ORIGIN = 0x08000000, LENGTH = 2048K\n RAM : ORIGIN = 0x20000000, LENGTH = 256K\n}\n").unwrap();
    println!("cargo:rustc-link-search={}", output.display());
    println!("cargo:rustc-link-arg-bin=openkeys-r05-diagnostics=-Tlink.x");
}
