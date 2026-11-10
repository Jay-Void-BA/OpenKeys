#!/usr/bin/env python3
"""Reject compiler future-incompatibility reports, even for capped dependencies."""
import argparse
import json
import os
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target-dir', type=Path,
                        default=Path(os.environ.get('CARGO_TARGET_DIR', 'target')))
    args = parser.parse_args()
    report = args.target_dir / '.future-incompat-report.json'
    if report.exists():
        data = json.loads(report.read_text())
        if not isinstance(data, dict) or not isinstance(data.get('reports'), list):
            raise SystemExit('Unknown Cargo future-incompatibility schema; review required')
        if data['reports']:
            raise SystemExit('Cargo recorded future-incompatible code; inspect '+str(report))
    print('No Cargo future-incompatibility reports recorded')


if __name__ == '__main__':
    main()
