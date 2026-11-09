#!/usr/bin/env python3
"""Verify the complete pinned OpenSK subset and its explicitly reviewed patches.

Original upstream hashes remain immutable in UPSTREAM.json. Local patches have
separate hashes and a distributable diff; no network or git checkout is needed.
"""
from pathlib import Path
import hashlib
import json
import sys

ROOT = Path(__file__).resolve().parents[1] / "vendor/opensk"
upstream = json.loads((ROOT / "UPSTREAM.json").read_text())
patches = json.loads((ROOT / "PATCHES.json").read_text())
original = upstream["unmodified_source_sha256"]
modified = patches["modified_source_sha256"]
errors = []
if not set(modified).issubset(original):
    errors.append("Patch manifest contains a file outside the upstream subset")
expected = original | modified
metadata = {"UPSTREAM.json", "PATCHES.json", "OPENKEYS.patch", "README.md"}
actual = {str(p.relative_to(ROOT)) for p in ROOT.rglob("*") if p.is_file()}
if actual != set(expected) | metadata:
    errors.append("Unexpected or missing vendored files: " + repr(sorted(actual ^ (set(expected) | metadata))))
for name, digest in expected.items():
    path = ROOT / name
    if not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != digest:
        errors.append("Source digest mismatch: " + name)
    if name in modified and path.is_file() and b"OpenKeys modifications" not in path.read_bytes()[:1500]:
        errors.append("Missing prominent modification notice: " + name)
if hashlib.sha256((ROOT / "OPENKEYS.patch").read_bytes()).hexdigest() != patches["patch_sha256"]:
    errors.append("Distributable patch digest mismatch")
for error in errors:
    print(error, file=sys.stderr)
if errors:
    sys.exit(1)
print(f"OpenSK {upstream['commit']}: {len(expected)} source files verified; {len(modified)} explicit patches")
