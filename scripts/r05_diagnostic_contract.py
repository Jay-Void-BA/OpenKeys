"""Source inventory for R05 public diagnostics only, not a production release."""
import hashlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FILES = ('Cargo.toml', 'Cargo.lock', 'scripts/export-r05-diagnostic.py',
         'scripts/check-r05-diagnostic.py', 'scripts/r05_diagnostic_contract.py',
         'scripts/freeze-r05-board.py')
FOLDERS = ('firmware/r05', 'firmware/board', 'firmware/vendor/cortex-m',
           'crates/device-policy', 'crates/device-vault', 'crates/se-transport')
HARDWARE = ('hardware/r05/kicad/OpenKeys.kicad_pcb', 'hardware/r05/pinout.csv')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sources():
    paths = {ROOT/name for name in FILES}
    for name in FOLDERS:
        paths.update(p for p in (ROOT/name).rglob('*') if p.is_file())
    return {str(p.relative_to(ROOT)): sha(p) for p in sorted(paths)}


def hardware():
    return {name: sha(ROOT/name) for name in HARDWARE}
