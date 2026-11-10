#!/usr/bin/env python3
"""Check local file/image links in first-party Markdown, without network I/O.

External URLs and heading fragments are not validated. Third-party source and
local scratch directories are outside the documentation inventory.
"""
from html.parser import HTMLParser
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
TOP_LEVEL = {'README.md', 'CONTRIBUTING.md', 'SECURITY.md'}
FOLDERS = {'docs', 'hardware', 'firmware', 'crates', 'apps', 'manufacturing'}
INLINE = re.compile(r'!?\[[^\]\n]*\]\(\s*(<[^>\n]+>|[^\s)]+)(?:\s+[\"\'][^\n]*?[\"\'])?\s*\)')
REFERENCE = re.compile(r'^\s{0,3}\[[^\]\n]+\]:\s*(<[^>\n]+>|[^\s]+)', re.MULTILINE)
FENCE = re.compile(r'^\s{0,3}(`{3,}|~{3,})')


class HtmlLinks(HTMLParser):
    def __init__(self):
        super().__init__()
        self.links = []

    def handle_starttag(self, tag, attrs):
        for key, value in attrs:
            if value and ((tag == 'img' and key == 'src') or (tag == 'a' and key == 'href')):
                self.links.append(value)


def outside_fences(text):
    lines = []
    fence = None
    for line in text.splitlines():
        match = FENCE.match(line)
        if match:
            marker = match.group(1)
            if fence is None:
                fence = marker
            elif marker[0] == fence[0] and len(marker) >= len(fence) and not line[match.end():].strip():
                fence = None
            continue
        if fence is None:
            lines.append(line)
    if fence is not None:
        raise ValueError('Unclosed Markdown code fence')
    return '\n'.join(lines)


def documents():
    paths = subprocess.check_output(
        ['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard', '--', '*.md'],
        cwd=ROOT,
    ).decode().split('\0')
    for name in sorted(set(paths)):
        path = Path(name)
        if name in TOP_LEVEL or (path.parts and path.parts[0] in FOLDERS):
            if not name.startswith('firmware/vendor/'):
                yield path


def main():
    errors = []
    files = list(documents())
    checked = 0
    for relative in files:
        try:
            text = outside_fences((ROOT / relative).read_text(encoding='utf-8'))
            parser = HtmlLinks()
            parser.feed(text)
            links = [m.group(1).strip('<>') for pattern in (INLINE, REFERENCE) for m in pattern.finditer(text)]
            links.extend(parser.links)
            for target in links:
                url = urlsplit(target)
                if url.scheme or url.netloc or not url.path:
                    continue
                path = unquote(url.path)
                destination = ((ROOT / path.lstrip('/')) if path.startswith('/')
                               else ROOT / relative.parent / path).resolve()
                if not destination.is_relative_to(ROOT):
                    errors.append(f'{relative}: link escapes repository: {target}')
                elif not destination.exists():
                    errors.append(f'{relative}: missing link target: {target}')
                checked += 1
        except (OSError, ValueError, UnicodeError) as error:
            errors.append(f'{relative}: {error}')
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        return 1
    print(f'Checked {len(files)} first-party Markdown files and {checked} local file/image links; external URLs and heading anchors excluded')
    return 0


if __name__ == '__main__':
    sys.exit(main())
