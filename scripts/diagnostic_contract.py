"""Inventory shared by diagnostic export and independent stale-file checks."""
import hashlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE_FILES = (
    'Cargo.toml', 'Cargo.lock', 'scripts/export-diagnostic.py',
    'scripts/check-diagnostic-manifest.py', 'scripts/diagnostic_contract.py',
    'scripts/export-se-state-layout.py', 'manufacturing/se-state-layout.json',
)
SOURCE_FOLDERS = (
    'firmware/board', 'firmware/target', 'firmware/secure-mmio',
    'firmware/vendor/cortex-m', 'crates/device-policy', 'crates/device-vault',
    'crates/se-transport', 'crates/scp03', 'crates/h573-crypto', 'crates/se-state',
)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_hashes():
    files = {ROOT / name for name in SOURCE_FILES}
    for folder in SOURCE_FOLDERS:
        files.update(p for p in (ROOT / folder).rglob('*') if p.is_file())
    return {str(p.relative_to(ROOT)): sha(p) for p in sorted(files)}
