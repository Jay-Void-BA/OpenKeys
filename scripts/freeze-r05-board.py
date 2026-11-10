#!/usr/bin/env python3
"""Bind firmware to an already independently checked saved board; never route."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FILES = ['hardware/r05/kicad/OpenKeys.kicad_pcb', 'hardware/r05/pinout.csv']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    check = json.loads((ROOT/'hardware/r05/review/board-check.json').read_text())
    actual = hashlib.sha256((ROOT/FILES[0]).read_bytes()).hexdigest()
    assert check['revision'] == '0.5' and check['board_sha256'] == actual
    assert check['drc_violations'] == 0 and check['unconnected'] == 0
    assert check['pcb_to_schematic_parity'] is True
    text = ''.join(hashlib.sha256((ROOT/name).read_bytes()).hexdigest()+'  '+name+'\n' for name in FILES)
    path = ROOT/'firmware/r05/board-contract.sha256'
    if args.check:
        if path.read_text() != text:
            raise SystemExit('R05 firmware board contract is stale')
    else:
        path.write_text(text)
    print('R05 PCB/pinout source contract checked; physical qualification remains NOT_RUN')


if __name__ == '__main__':
    main()
