#!/usr/bin/env python3
"""Independent public vectors; never device provisioning.

Requires Python cryptography, not a runtime or CI dependency. Every key,
nonce and plaintext below is public synthetic fixture data, never a real key.
"""
import json

import cryptography
from cryptography.hazmat.primitives.ciphers.aead import AESGCM


def vector(header, body, iv):
    sealed = AESGCM(bytes([0x35]) * 32).encrypt(iv, body, header)
    return (header + iv + sealed[-16:] + sealed[:-16]).hex()


def main():
    outbound = vector(bytes.fromhex("0400110011002600"),
                      bytes.fromhex("55001100343007000100"), bytes(range(12)))
    inbound = vector(bytes.fromhex("0400130041002800"),
                     bytes.fromhex("55001300ec61343007000100"), bytes(range(16, 28)))
    assert outbound == "0400110011002600000102030405060708090a0b9b9862a09a163b217bfb77611c2d28566d35f45cf3c34dfce633"
    assert inbound == "0400130041002800101112131415161718191a1b30404c6a465f45d2f2589b7fae6e82748f678e2c8bc58f3af08ce47a"
    print(json.dumps({"generator": "cryptography " + cryptography.__version__,
                      "purpose": "PUBLIC_TEST_FIXTURES_ONLY", "key_hex": "35" * 32,
                      "outbound_hex": outbound, "inbound_hex": inbound}, indent=2))


if __name__ == "__main__":
    main()
