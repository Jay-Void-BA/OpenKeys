//! PUBLIC synthetic material only. Not a real-vault authorization bypass.
use std::time::Instant;
fn main() {
    let item = r#"{"type":"note","title":"Public example","body":"Synthetic benchmark"}"#;
    let data = format!(
        "{{\"version\":2,\"entries\":[{}]}}",
        vec![item; 256].join(",")
    );
    let start = Instant::now();
    for _ in 0..200 {
        let entries = vault_import::parse_import(std::hint::black_box(data.as_bytes())).unwrap();
        std::hint::black_box(vault_import::manifest_digest(&entries));
    }
    println!(
        "PUBLIC benchmark: 51200 notes parsed and hashed in {:?}",
        start.elapsed()
    );
}
