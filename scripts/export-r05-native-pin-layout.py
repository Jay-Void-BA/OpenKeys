#!/usr/bin/env python3
"""Export/check native PIN v4 public contract. Never connects or programs a key."""
import argparse
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def layout():
    pin = ROOT / 'crates/se-state/src/native_pin.rs'
    legacy = ROOT / 'crates/se-state/src/pin_only.rs'
    source = pin.read_text()
    ids = [('PIN', 0x4F4B0801, 'userid', 6),
           ('OWNER', 0x4F4B0802, 'binary', 32),
           ('SPENT', 0x4F4B0501, 'counter', 8),
           ('CREDIT', 0x4F4B0502, 'counter', 8),
           ('ERASE', 0x4F4B0507, 'counter', 8),
           ('MARKER', 0x4F4B0506, 'binary', 32)]
    for role, value, _, _ in ids:
        text = source if role in ('PIN', 'OWNER') else legacy.read_text()
        m = re.search(r'const ' + role + r'_ID: u32 = (0x[0-9a-f_]+);', text)
        assert m and int(m[1].replace('_', ''), 16) == value
    assert 'pub const MAX_FAILURES: u64 = 11;' in legacy.read_text()
    policy = bytes.fromhex('1112084f4b080110260000080000000020000000')
    data = (b'OpenKeys/R05/native-PIN-state/v4' + bytes([6]) + (11).to_bytes(8, 'big')
            + b''.join(value.to_bytes(4, 'big') for _, value, _, _ in ids) + policy
            + b'immutable-userid/immutable-owner-epoch/credit-owner-only/platform-SCP03-33/marker-last')
    marker = hashlib.sha256(data).hexdigest()
    tests = (ROOT / 'crates/se-state/src/native_pin_tests.rs').read_text()
    assert marker in tests, 'Rust native marker fixture does not match independent encoding'
    return {
        'schema': 1, 'revision': '0.5', 'native_pin_profile': 4,
        'status': 'CANDIDATE_NOT_COMMISSIONED_NOT_QUALIFIED',
        'production_root_integrated': False, 'commissioner_implemented': False,
        'pin_namespace_installer_implemented': True,
        'all_data_erasure_implemented': False, 'programming_performed': False,
        'contains_production_secrets': False, 'pin_decimal_digits': 6,
        'maximum_failed_or_ambiguous_completed_guesses': 11,
        'objects': [dict(role=r, id=f'{v:08X}', kind=k, bytes=n, persistent=True)
                    for r, v, k, n in ids],
        'marker_sha256': marker, 'signing_owner_policy_tlv_hex': policy.hex(),
        'required_acl': {
            'PIN': 'Explicit auth flag + maxAttempts11; immutable/no delete or rebind; full platform SCP03',
            'OWNER': 'Immutable/no delete or rewrite; compare fresh SE nonce with authenticated Root journal before verification and every private request; fresh random nonce each full wipe',
            'SPENT': 'Monotonic precharge; no decrement/reset/delete',
            'CREDIT': 'Only PIN-authenticated native session may WRITE; platform may READ only',
            'ERASE': 'Monotonic nonzero latch refuses ordinary access; survives until qualified ALL-DATA wipe',
            'MARKER': 'Written LAST; no runtime WRITE/DELETE; digest is not an ACL proof',
            'ALL_PRIVATE_OBJECTS': 'Explicit PIN AuthID policy, no default/all-other-user private operations',
        },
        'native_userid_counter_readback': 'Not a retry counter: AN12413 3.2.4.4 says UserID reports zero',
        'userid_read_size': 'Not supported; never used as a length/PIN-strength proof',
        'migration': 'No automatic migration from legacy HKDF v2 or native v3; full verified destruction first',
        'physical_results': 'NOT_RUN',
        'source_sha256': {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
                          for p in [pin, legacy]},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    result = layout()
    text = json.dumps(result, indent=2) + '\n'
    path = ROOT / 'manufacturing/se-native-pin-layout-r05.json'
    if args.check:
        if not path.is_file() or path.read_text() != text:
            raise SystemExit('Native PIN v4 public contract is stale')
    else:
        path.write_text(text)
    print('Native PIN v4 contract checked; NOT programmed/qualified; marker=' + result['marker_sha256'])


if __name__ == '__main__':
    main()
