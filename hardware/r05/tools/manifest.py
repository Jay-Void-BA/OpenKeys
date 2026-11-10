#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""Hash the R05 CAD candidate; this is not a production release signature."""
import argparse
import hashlib
import json
from pathlib import Path
import zipfile

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / 'review/cad-manifest.json'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    if args.check:
        data = json.loads(MANIFEST.read_text())
        for relative, expected in data['sha256'].items():
            path = (ROOT / relative).resolve()
            if not path.is_relative_to(ROOT) or digest(path) != expected:
                raise SystemExit('CAD manifest mismatch: ' + relative)
        print(f"Verified {len(data['sha256'])} CAD artifact hashes")
        return
    board_check = json.loads((ROOT / 'review/board-check.json').read_text())
    assert board_check['board_sha256'] == digest(ROOT / 'kicad/OpenKeys.kicad_pcb')
    assert board_check['drc_violations'] == 0 and board_check['unconnected'] == 0
    erc = json.loads((ROOT / 'review/erc.json').read_text())
    assert not any(sheet['violations'] for sheet in erc['sheets'])
    destination = ROOT / 'exports/OpenKeys-fabrication.zip'
    with zipfile.ZipFile(destination, 'w', zipfile.ZIP_DEFLATED) as archive:
        for path in sorted((ROOT / 'exports/fabrication').iterdir()):
            if path.is_file():
                entry = zipfile.ZipInfo(path.name, (2026, 11, 10, 0, 0, 0))
                entry.compress_type = zipfile.ZIP_DEFLATED
                entry.external_attr = 0o644 << 16
                archive.writestr(entry, path.read_bytes())
    files = [ROOT / 'BOM.csv', ROOT / 'pinout.csv']
    for folder in ('kicad', 'exports', 'mechanical', 'tools', 'simulation'):
        files.extend(path for path in (ROOT / folder).rglob('*')
                     if path.is_file() and path.suffix not in ('.blend1', '.kicad_prl', '.lck', '.pyc'))
    files.extend(ROOT / 'review' / name for name in (
        'erc.json', 'routed-drc.json', 'netlist-check.json', 'board-check.json',
        'simulation.json', 'protection-calculations.json', 'display-power-model.json', 'ground-fanout.json'))
    report = {
        'schema': 1, 'revision': '0.5', 'status': 'CAD_CANDIDATE',
        'routing_complete': True, 'electrical_checks_passed': True,
        'production_release': False, 'physical_qualification': False,
        'security_certification': False,
        'limits': 'Hashes bind these files only. DRC, passive simulations and drawings '
                  'do not qualify assembly, firmware, EMC, fault or side-channel resistance.',
        'sha256': {str(path.relative_to(ROOT)): digest(path) for path in sorted(set(files))},
    }
    MANIFEST.write_text(json.dumps(report, indent=2) + '\n')
    print(f"Recorded {len(report['sha256'])} CAD artifact hashes; production release remains false")


if __name__ == '__main__':
    main()
