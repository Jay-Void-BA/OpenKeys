#!/usr/bin/env python3
"""Public R05 PIN namespace only. No keys, APDUs, fuses or programming."""
import argparse
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OBJECTS = [('SPENT', 0x4F4B0501, 'counter', 8),
           ('CREDIT', 0x4F4B0502, 'counter', 8),
           ('SALT', 0x4F4B0503, 'binary', 32),
           ('VERIFIER', 0x4F4B0504, 'binary', 32),
           ('PEPPER', 0x4F4B0505, 'hmac-key', 32),
           ('MARKER', 0x4F4B0506, 'binary', 32),
           ('ERASE', 0x4F4B0507, 'counter', 8)]
INFO = b'OpenKeys/R05/PIN-check/v2'


def layout():
    source = (ROOT / 'crates/se-state/src/pin_only.rs').read_text()
    for role, value, _, _ in OBJECTS:
        match = re.search(r'const ' + role + r'_ID: u32 = (0x[0-9a-f_]+);', source)
        assert match and int(match[1].replace('_', ''), 16) == value
    assert 'pub const MAX_FAILURES: u64 = 11;' in source
    assert f'b"{INFO.decode()}"' in source
    marker = hashlib.sha256(
        b'OpenKeys/R05/PIN-state/v2' + bytes([6]) + (11).to_bytes(8, 'big') + INFO
        + b'HKDF-SHA256/salt32/info-domain-decimal6'
        + b''.join(value.to_bytes(4, 'big') for _, value, _, _ in OBJECTS)).hexdigest()
    return {
        'schema': 1, 'revision': '0.5', 'status': 'NOT_COMMISSIONED_NOT_RELEASED',
        'fingerprint': False, 'pin_decimal_digits': 6, 'consecutive_attempt_limit': 11,
        'attempt_charged_before_verification': True, 'incomplete_entry_charged': False,
        'ambiguous_verification_refunds': False, 'erase_on_eleventh_wrong_guess': True,
        'erase_on_hardware_pin_change': True, 'erase_completion_implemented': False,
        'programming_performed': False, 'contains_production_secrets': False,
        'objects': [dict(role=role, id=f'{value:08X}', kind=kind, bytes=size,
                         persistent=True) for role, value, kind, size in OBJECTS],
        'kdf': 'RFC5869 HKDF-SHA256 inside SE; secret IKM=unique random pepper32; '
               'salt=random32; info=domain || 6 numeric bytes 0..9; L=32',
        'kdf_domain': INFO.decode(), 'marker_sha256': marker,
        'required_acl': [
            'Mandatory authenticated full SCP03; unique per-device ENC/MAC/DEK',
            'Pepper non-exportable; salt/verifier never available to host or external flash',
            'No unauthorized object overwrite/delete/recreate/counter reset',
            'Spent increments before completed-guess check; credit only after successful local verification',
            'A compromised authorized root remains a security boundary failure',
        ],
        'commissioning_constraints': [
            'Reviewed production root and closed debug required BEFORE owner PIN entry',
            'First boot requires two matching local six-digit entries; no factory/default PIN or host setup',
            'Owner enters PIN locally; ten isolated GPIO; no host PIN endpoint',
            'Refuse any pre-existing/partial namespace; marker written LAST',
            'Actual SE050E2 APDUs, ACLs, erase and counter power-loss semantics NOT qualified',
            'Legacy R04 counters/biometrics do not migrate or enable the R05 profile',
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    result = layout()
    path = ROOT / 'manufacturing/se-pin-layout-r05.json'
    data = json.dumps(result, indent=2) + '\n'
    if args.check:
        if not path.is_file() or path.read_text() != data:
            raise SystemExit('R05 public PIN layout is stale')
    else:
        path.write_text(data)
    print('Public R05 schema checked; no programming; marker=' + result['marker_sha256'])


if __name__ == '__main__':
    main()
