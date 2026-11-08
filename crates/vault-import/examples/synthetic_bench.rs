//! PUBLIC RFC test material only. Not a real-vault import bypass.
use std::time::Instant;
fn main() {
    let uri = "otpauth://totp/Example:alice?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=Example";
    let data = vec![uri; 256].join("\n");
    let start = Instant::now();
    for _ in 0..200 {
        let entries = vault_import::parse_import(std::hint::black_box(data.as_bytes())).unwrap();
        std::hint::black_box(vault_import::manifest_digest(&entries));
    }
    println!(
        "PUBLIC synthetic benchmark: 51200 entries parsed + hashed in {:?}",
        start.elapsed()
    );
}
