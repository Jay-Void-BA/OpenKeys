#!/usr/bin/env python3
"""Export PUBLIC namespace/schema data, never sends APDUs or provisions keys.

This is a manufacturing-review input, not authorization to program a device.
It intentionally contains no default/factory/production keys or fuses.
"""
import hashlib
import json
import argparse
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
COUNTERS = [('biometric_spent', 0x4F4B0001), ('biometric_credit', 0x4F4B0002),
            ('biometric_tag', 0x4F4B0003), ('vault_nonce', 0x4F4B0004), ('vault_commit', 0x4F4B0005),
            ('fido_signature', 0x4F4B0006), ('signing_slot', 0x4F4B0007)]
FILES = [('vault_head0', 0x4F4B0100), ('vault_head1', 0x4F4B0101), ('namespace_marker', 0x4F4B0102)]


def layout():
    domain = b'OpenKeys/R04/SE-state/v2'
    policy = ((8).to_bytes(8, 'big') + (65535).to_bytes(2, 'big')
              + (1048576).to_bytes(4, 'big') + (104).to_bytes(4, 'big') + (256).to_bytes(2, 'big'))
    addresses = b''.join(value.to_bytes(4, 'big') for _, value in COUNTERS + FILES)
    marker = hashlib.sha256(domain + policy + addresses).digest()
    empty = b'OpenKeysHeadV1\0\0' + bytes(56)
    assert len(empty) == 72
    empty += hashlib.sha256(empty).digest()
    return {
        'schema': 2, 'status': 'ENGINEERING_SCHEMA_NOT_PRODUCTION_APPROVAL',
        'programming_performed': False, 'contains_private_keys_or_fuses': False,
        'persistent_type': 1, 'biometric_consecutive_failure_limit': 8,
        'pin_only_recovery': False, 'maximum_identify_tag': 65535,
        'recovery_procedure_qualified': False,
        'fido_signature_maximum': 4294967295, 'signing_slots_maximum': 256,
        'automatic_migration_or_counter_reset': False,
        'counter_objects': [dict(role=name, id=f'{value:08X}', bytes=8,
                                 initial_value='0000000000000000') for name, value in COUNTERS],
        'file_objects': [dict(role=name, id=f'{value:08X}', bytes=32 if index == 2 else 104,
                              initial_value=(marker if index == 2 else empty).hex().upper())
                         for index, (name, value) in enumerate(FILES)],
        'marker_sha256': marker.hex(),
        'marker_purpose': 'Binds policy/layout, not peer identity or proof of correct ACLs',
        'required_external_checks': [
            'Exact E2 applet/OEF and persistent counter power-loss semantics qualified',
            'Unique per-device SCP ENC/MAC/DEK, authenticated root provisioning and mandatory full SCP',
            'Actual object policies verified; no unauthorized delete, reset, export or recreate',
            'Refuse commissioning if any namespace object already exists; no runtime creation',
            'Write marker LAST, after successful readback and controlled commissioning',
            'Secure boot/update/debug/tamper and memory/DMA attribution deployed',
        ],
    }


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='Verify saved public schema without writing')
    args=parser.parse_args()
    result = layout()
    path=ROOT / 'manufacturing/se-state-layout.json'
    encoded=json.dumps(result, indent=2) + '\n'
    if args.check:
        if not path.is_file() or path.read_text()!=encoded:
            raise SystemExit('Public SE schema mismatch; regenerate and review')
    else:
        path.write_text(encoded)
    print('Public schema only; marker=' + result['marker_sha256'] + '; no programming performed')
