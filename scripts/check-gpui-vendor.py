#!/usr/bin/env python3
"""Check GPUI support forks and optionally reconstruct from original crates."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / 'vendor/gpui-support'
MANIFEST = BASE / 'manifest.json'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def ignored(path):
    return (path.name in {'.cargo-checksum.json', '.cargo-ok', 'Cargo.lock'}
            or '.github' in path.parts or 'target' in path.parts)


def inventory():
    return {str(p.relative_to(BASE)): sha(p.read_bytes())
            for p in sorted(BASE.rglob('*')) if p.is_file() and p != MANIFEST and not ignored(p)}


def reconstruct(records, archives):
    for record in records:
        name = record['name'] + '-' + record['version']
        if archives:
            blob = (archives / (name + '.crate')).read_bytes()
        else:
            with urllib.request.urlopen(record['url'], timeout=60) as response:
                blob = response.read(16 * 1024 * 1024 + 1)
        assert len(blob) <= 16 * 1024 * 1024 and sha(blob) == record['crate_sha256'], name
        with tempfile.TemporaryDirectory(prefix='openkeys-vendor-') as temporary:
            folder = Path(temporary)
            with tarfile.open(fileobj=io.BytesIO(blob)) as archive:
                for member in archive.getmembers():
                    destination = (folder / member.name).resolve()
                    assert destination.is_relative_to(folder) and not member.issym() and not member.islnk()
                archive.extractall(folder, filter='data')
            patch = BASE / 'patches' / (name + '.patch')
            subprocess.run(['patch', '--batch', '--fuzz=0', '-p1', '-i', str(patch)],
                           cwd=folder, check=True, stdout=subprocess.DEVNULL)
            original = {str(p.relative_to(folder/name)): sha(p.read_bytes())
                        for p in (folder/name).rglob('*') if p.is_file() and not ignored(p)}
            delivered = {str(p.relative_to(BASE/name)): sha(p.read_bytes())
                         for p in (BASE/name).rglob('*') if p.is_file() and not ignored(p)}
            assert original == delivered, 'Upstream reconstruction mismatch: ' + name
    print(f'Reconstructed {len(records)} forks from SHA-256-pinned crates with exact patches')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--refresh', action='store_true')
    parser.add_argument('--verify-upstream', action='store_true')
    parser.add_argument('--archives', type=Path, help='Optional local crates cache for upstream verification')
    args = parser.parse_args()
    records = json.loads((BASE/'UPSTREAM.json').read_text())
    current = inventory()
    if args.refresh:
        MANIFEST.write_text(json.dumps({'schema': 1, 'sha256': current}, indent=2)+'\n')
    else:
        assert json.loads(MANIFEST.read_text())['sha256'] == current, 'GPUI source inventory mismatch'
    if args.verify_upstream:
        reconstruct(records, args.archives)
    print(f'Checked {len(records)} GPUI support forks and {len(current)} source/patch hashes')


if __name__ == '__main__':
    main()
