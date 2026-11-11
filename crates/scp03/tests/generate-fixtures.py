#!/usr/bin/env python3
"""Public, non-production SCP03 fixtures using the external OpenSSL CLI.

Development-only oracle: GP Amendment D / NXP nano-package 26d55b4.
Nothing here is a device key, runtime dependency or provisioning tool.
Run this file, then cargo fmt; normal tests use the checked-in fixed vectors.
"""
from pathlib import Path
import subprocess


def run(args, data):
    return subprocess.check_output(['openssl', *args], input=data)


def mac(key, data):
    return run(['mac', '-macopt', f'cipher:AES-{len(key)*8}-CBC',
                '-macopt', 'hexkey:' + key.hex(), '-binary', 'CMAC'], data)


def aes(key, data, iv=None):
    args = ['enc', f'-aes-{len(key)*8}-' + ('ecb' if iv is None else 'cbc'),
            '-K', key.hex(), '-nopad', '-nosalt']
    if iv is not None:
        args += ['-iv', iv.hex()]
    return run(args, data)


def derive(key, label, context, length):
    result = b''
    for counter in range(1, (length + 15) // 16 + 1):
        result += mac(key, bytes(11) + bytes([label, 0])
                      + (length * 8).to_bytes(2, 'big') + bytes([counter]) + context)
    return result[:length]


def pad(data):
    return data + b'\x80' + bytes((15 - len(data)) % 16)


def fixture(size, pseudo):
    master_enc = bytes(range(size))
    master_mac = bytes(range(32, 32 + size))
    host = bytes.fromhex('0102030405060708')
    aid = bytes.fromhex('A000000151000000')  # Public GP example; NOT an NXP profile.
    sequence = bytes.fromhex('000021')
    card = derive(master_enc, 2, sequence + aid, 8) if pseudo else bytes.fromhex('1011121314151617')
    context = host + card
    enc = derive(master_enc, 4, context, size)
    cm = derive(master_mac, 6, context, size)
    rm = derive(master_mac, 7, context, size)
    response = bytes(10) + bytes([1, 3, 0x70 if pseudo else 0x60]) + card
    response += derive(cm, 0, context, 8) + (sequence if pseudo else b'') + b'\x90\0'
    ea = bytes.fromhex('8482330010') + derive(cm, 1, context, 8)
    chain = mac(cm, bytes(16) + ea)
    ea += chain[:8]
    fields = {'INITIALIZE': bytes.fromhex('8050010008') + host + b'\0',
              'IU_RESPONSE': response, 'EXTERNAL': ea,
              'SESSION_ENC': enc, 'SESSION_MAC': cm, 'SESSION_RMAC': rm}
    cases = [(b'', b'', ('short', 0)),
             (b'public test data', b'public response!', ('short', 0)),
             (bytes(range(239)), bytes(reversed(range(239))), ('short', 0)),
             (bytes(range(240)), bytes(i % 256 for i in range(512)), ('extended', 1024)),
             (bytes(i % 256 for i in range(2015)), bytes((i * 7) % 256 for i in range(2015)), ('extended', 0)),
             (b'X', bytes(i % 256 for i in range(512)), ('extended', 512)),
             (bytes(range(240)), b'', ('short', 0)),
             (bytes(range(240)), b'', None)]
    for counter, (data, reply, le) in enumerate(cases, 1):
        command_iv = aes(enc, counter.to_bytes(16, 'big'))
        cipher = aes(enc, pad(data), command_iv) if data else b''
        lc = len(cipher) + 8
        extended = lc > 255 or (le is not None and le[0] == 'extended')
        length = b'\0' + lc.to_bytes(2, 'big') if extended else bytes([lc])
        command = bytes([0x84, 0xCA, 0, 0]) + length + cipher
        chain = mac(cm, chain + command)
        encoded_le = b''
        if le is not None:
            value = 256 if le == ('short', 0) and extended else le[1]
            encoded_le = value.to_bytes(2 if extended else 1, 'big')
        fields[f'COMMAND_{counter}'] = command + chain[:8] + encoded_le
        response_iv = aes(enc, b'\x80' + bytes(11) + counter.to_bytes(4, 'big'))
        cipher = aes(enc, pad(reply), response_iv) if reply else b''
        fields[f'RESPONSE_{counter}'] = cipher + mac(rm, chain + cipher + b'\x90\0')[:8] + b'\x90\0'
    return fields


def render():
    version = subprocess.check_output(['openssl', 'version'], text=True).strip()
    output = '// Public TEST VECTORS ONLY. Generated with ' + version + '.\n'
    for name, size, pseudo in [('AES128_RANDOM', 16, False), ('AES128_PSEUDO', 16, True),
                               ('AES256_RANDOM', 32, False)]:
        output += f'pub mod {name.lower()} {{\n'
        for field, value in fixture(size, pseudo).items():
            byte_list = ', '.join(f'0x{x:02x}' for x in value)
            output += f'pub const {field}: [u8; {len(value)}] = [{byte_list}];\n'
        output += '}\n'
    Path(__file__).resolve().parents[1].joinpath('src/test_vectors.rs').write_text(output)


if __name__ == '__main__':
    render()
