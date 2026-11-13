#!/usr/bin/env python3
"""Check the non-dev ARM graph, not the host test graph or physical security."""
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]
try:
    output = subprocess.check_output([
        'cargo', '+1.99.1', 'tree', '--locked',
        '-p', 'openkeys-authenticator-core',
        '--target', 'thumbv8m.main-none-eabihf',
        '--edges', 'normal,build', '--prefix', 'none', '--format', '{p}|{f}',
    ], cwd=root, text=True)
except FileNotFoundError:
    raise SystemExit('cargo unavailable: install Rust1.99.1 and the thumbv8m.main-none-eabihf target') from None
matches = [line for line in output.splitlines() if line.startswith('opensk v')]
if len(matches) != 1:
    raise SystemExit('Expected exactly one OpenSK dependency in the classical ARM graph')
features = set(matches[0].split('|', 1)[1].split(','))
if features != {'ctap1', 'openkeys-local-pin'}:
    raise SystemExit('Unreviewed OpenSK features in classical ARM graph: ' + repr(sorted(features)))
print('Classical ARM graph: ctap1 + openkeys-local-pin; physical qualification remains NOT_RUN')
