use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var_os("CARGO_FEATURE_R04_DIAGNOSTICS").is_none() {
        return;
    }
    assert_eq!(env::var("TARGET").unwrap(), "thumbv8m.main-none-eabihf");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    // OPEN/TZEN=0 bring-up only. Production STiRoT images have a distinct
    // verified header/address/security profile and must never use this map.
    fs::write(output.join("memory.x"), "MEMORY {\n FLASH : ORIGIN = 0x08000000, LENGTH = 2048K\n RAM : ORIGIN = 0x20000000, LENGTH = 256K\n}\n").unwrap();
    println!("cargo:rustc-link-search={}", output.display());
    println!("cargo:rustc-link-arg-bin=openkeys-r04-diagnostics=-Tlink.x");
}
