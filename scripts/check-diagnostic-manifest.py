#!/usr/bin/env python3
"""Reject stale diagnostic sources/artifacts; does not build or flash a device."""
import json
import struct
import sys
from diagnostic_contract import ROOT, sha, source_hashes


def require(condition, message):
    if not condition:
        raise ValueError(message)


def check():
    dist = ROOT / 'firmware/dist'
    report = json.loads((dist / 'diagnostic-manifest.json').read_text())
    require(report.get('schema') == 1, 'Unknown diagnostic manifest schema')
    require(report.get('purpose') == 'R04_ELECTRICAL_DIAGNOSTIC_ONLY', 'Wrong image purpose')
    require(report.get('target') == 'thumbv8m.main-none-eabihf', 'Wrong diagnostic target')
    require(report.get('boot_profile') == 'OPEN lifecycle, TZEN=0; NOT production STiRoT',
            'Wrong diagnostic boot profile')
    for key in ('production_authenticator', 'signed_update', 'provisioning_or_flashing_performed',
                'fido', 'vault', 'charging_enabled'):
        require(report.get(key) is False, f'Unsafe or missing diagnostic flag: {key}')
    require(report.get('sensor_and_se_disabled') is True, 'Diagnostic must disable sensor and SE')
    require(report.get('physical_test_result') == 'NOT_RUN', 'Export is not a physical test')
    expected = source_hashes()
    actual = report.get('source_sha256', {})
    changed = sorted(k for k in expected.keys() | actual.keys() if expected.get(k) != actual.get(k))
    require(not changed, 'Stale source inventory/digest: ' + ', '.join(changed))
    hardware = {
        'pcb': sha(ROOT / 'hardware/r04/kicad/OpenKeys.kicad_pcb'),
        'pinout': sha(ROOT / 'hardware/r04/pinout.csv'),
    }
    require(report.get('hardware_sha256') == hardware, 'Stale PCB or pinout digest')
    names = ('openkeys-r04-diagnostics.elf', 'openkeys-r04-diagnostics.bin')
    require(report.get('artifacts') == {name: sha(dist / name) for name in names},
            'Diagnostic artifact set or bytes do not match')
    elf, binary = ((dist / name).read_bytes() for name in names)
    require(elf[:7] == b'\x7fELF\x01\x01\x01' and elf[18:20] == b'\x28\x00', 'Wrong ELF architecture')
    require(8 <= len(binary) <= 2 * 1024 * 1024, 'Invalid diagnostic BIN length')
    stack, reset = struct.unpack_from('<II', binary)
    require(0x20000000 < stack <= 0x20040000 and reset & 1, 'Invalid ARM vector table')
    require(0x08000000 <= reset < 0x08000000 + len(binary), 'Reset vector outside image')
    require(report.get('binary_bytes') == len(binary) and report.get('initial_stack') == hex(stack)
            and report.get('reset_vector') == hex(reset), 'Stale vector/length metadata')
    print(f'Verified diagnostic only: {len(expected)} source hashes; ARM artifacts and PCB/pinout match')


if __name__ == '__main__':
    try:
        check()
    except (ValueError, OSError, KeyError, TypeError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
