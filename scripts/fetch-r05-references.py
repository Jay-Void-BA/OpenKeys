#!/usr/bin/env python3
"""Fetch fixed read-only study checkouts. Never resets an existing checkout."""
import argparse
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def git(*args, cwd=None):
    return subprocess.check_output(['git', *args], cwd=cwd, text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('destination', type=Path, help='Separate study folder, outside this repository')
    args = parser.parse_args()
    destination = args.destination.resolve()
    if destination == ROOT or destination.is_relative_to(ROOT):
        raise SystemExit('Use an external study folder; do not mix checkouts into the project')
    destination.mkdir(parents=True, exist_ok=True)
    records = json.loads((ROOT/'references-r05.lock.json').read_text())['repositories']
    for record in records:
        name, repository, commit = (record[k] for k in ('name', 'repository', 'commit'))
        assert re.fullmatch(r'[a-z0-9-]+', name) and re.fullmatch(r'[a-f0-9]{40}', commit)
        assert re.fullmatch(r'https://github.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repository)
        path = destination/name
        if path.exists():
            if git('status', '--porcelain', cwd=path):
                raise SystemExit(f'{path}: existing checkout is modified; left unchanged')
            if git('rev-parse', 'HEAD', cwd=path) != commit:
                raise SystemExit(f'{path}: existing checkout differs; left unchanged')
            if git('remote', 'get-url', 'origin', cwd=path).removesuffix('.git') != repository:
                raise SystemExit(f'{path}: origin differs; left unchanged')
            print(name+': already at locked commit; existing files left unchanged')
            continue
        path.mkdir()
        subprocess.run(['git', 'init', '--initial-branch=main', str(path)], check=True, stdout=subprocess.DEVNULL)
        subprocess.run(['git', 'remote', 'add', 'origin', repository], cwd=path, check=True)
        subprocess.run(['git', 'fetch', '--depth=1', 'origin', commit], cwd=path, check=True)
        subprocess.run(['git', 'checkout', '--detach', 'FETCH_HEAD'], cwd=path, check=True)
        assert git('rev-parse', 'HEAD', cwd=path) == commit
        print(name+': checked out '+commit+'; no submodules/build/tests executed')


if __name__ == '__main__':
    main()
