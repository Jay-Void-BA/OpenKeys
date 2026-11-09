#[path = "src/pins.rs"]
mod pins;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

fn main() {
    assert_eq!(pins::BOARD_REVISION, "0.4");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, expected) in [
        ("hardware/r04/kicad/OpenKeys.kicad_pcb", pins::BOARD_SHA256),
        ("hardware/r04/pinout.csv", pins::PINOUT_SHA256),
    ] {
        let path = root.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        let bytes = fs::read(&path).expect("Frozen R04 hardware artifact is missing");
        let observed = format!("{:x}", Sha256::digest(&bytes));
        assert_eq!(
            observed, expected,
            "R04 hardware changed: review drivers and freeze a new board contract"
        );
    }
    let csv = fs::read_to_string(root.join("hardware/r04/pinout.csv")).unwrap();
    let mut count = 0;
    for row in csv.lines().skip(1) {
        let cells: Vec<_> = row.split(',').collect();
        let gpio = cells[1].as_bytes();
        if gpio.first() != Some(&b'P') {
            continue;
        }
        let (_, pin) = pins::PIN_MAP
            .iter()
            .find(|(name, _)| *name == cells[2])
            .expect("Missing GPIO in board contract");
        assert_eq!(pin.package_pin, cells[0].parse::<u8>().unwrap());
        assert_eq!(pin.port, gpio[1]);
        assert_eq!(pin.bit, cells[1][2..].parse::<u8>().unwrap());
        let alternate = cells[3]
            .split("AF")
            .nth(1)
            .map(|s| s.parse::<u8>().unwrap());
        assert_eq!(pin.alternate, alternate);
        count += 1;
    }
    assert_eq!(count, pins::PIN_MAP.len());
}
