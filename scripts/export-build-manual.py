#!/usr/bin/env python3
"""Render the Chinese engineering manual to PDF, or check its source binding.

Rendering uses installed Pandoc, WeasyPrint and Noto CJK fonts. --check uses only
Python's standard library and does not rebuild, flash or provision hardware.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
from urllib.parse import quote, unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
SOURCE = 'manufacturing/BUILD_MANUAL_ZH.md'
CSS = 'manufacturing/manual-print.css'
PDF = 'manufacturing/OpenKeys-R04-build-manual-zh.pdf'
MANIFEST = 'manufacturing/build-manual-manifest.json'
INPUTS = (
    SOURCE, CSS, 'manufacturing/manual-renderer-requirements.txt', 'manufacturing/ORDER_SHEET_R04.md',
    'manufacturing/procurement-r04-board.csv',
    'manufacturing/procurement-r04-external.csv',
    'manufacturing/procurement-r04-manifest.json',
    'scripts/export-build-manual.py', 'scripts/export-manufacturing-worksheet.py',
    'hardware/r04/BOM.csv', 'hardware/r04/pinout.csv',
    'hardware/r04/exports/OpenKeys-fabrication.zip',
)


def sha(name):
    return hashlib.sha256((ROOT / name).read_bytes()).hexdigest()


def sources():
    return {name: sha(name) for name in INPUTS}


def verify():
    report = json.loads((ROOT / MANIFEST).read_text())
    if report.get('schema') != 1 or report.get('production_release') is not False:
        raise ValueError('Invalid manual provenance or release status')
    if report.get('purpose') != 'ENGINEERING_BUILD_INSTRUCTIONS_ONLY':
        raise ValueError('Invalid manual purpose')
    if report.get('source_sha256') != sources():
        raise ValueError('Manual source changed: re-render and visually review the PDF')
    if report.get('pdf_sha256') != sha(PDF):
        raise ValueError('Manual PDF digest mismatch')
    data = (ROOT / PDF).read_bytes()
    if not data.startswith(b'%PDF-') or len(data) != report.get('pdf_bytes') or len(data) < 50_000:
        raise ValueError('Invalid manual PDF')
    print(f'Verified engineering manual source/PDF binding: {len(data)} bytes; no production release')


def github_links(markdown):
    def replace(match):
        prefix, target = match.group(1), match.group(2)
        url = urlsplit(target)
        if url.scheme or url.netloc:
            return match.group(0)
        if not url.path:
            relative = Path(SOURCE)
        else:
            destination = ((ROOT / SOURCE).parent / unquote(url.path)).resolve()
            if not destination.is_relative_to(ROOT):
                raise ValueError('Manual link escapes repository')
            relative = destination.relative_to(ROOT)
        base = 'tree' if (ROOT / relative).is_dir() else 'blob'
        address = f'https://github.com/Jay-Void-BA/OpenKeys/{base}/main/' + quote(relative.as_posix(), safe='/')
        if url.fragment:
            address += '#' + quote(url.fragment, safe='-')
        return prefix + address + ')'
    return re.sub(r'(\[[^\]\n]+\]\()([^\s)]+)\)', replace, markdown)


def render():
    from importlib.metadata import version
    try:
        from weasyprint import HTML, URLFetcher
    except ImportError as error:
        raise ValueError('Install the pinned manual renderer in an isolated Python environment') from error
    if not shutil.which('pandoc'):
        raise ValueError('Missing manual renderer: pandoc')
    before = sources()
    with tempfile.TemporaryDirectory(prefix='openkeys-manual-') as temp:
        folder = Path(temp)
        markdown = folder / 'manual.md'
        content = github_links((ROOT / SOURCE).read_text())
        if content.startswith('# OpenKeys R0.4 制作说明书\n'):
            content = content.split('\n', 1)[1].lstrip()
        markdown.write_text(content)
        html = folder / 'manual.html'
        subprocess.run([
            'pandoc', str(markdown), '--from=gfm', '--to=html5', '--standalone',
            '--toc', '--toc-depth=2', '--metadata=title:OpenKeys R0.4 制作说明书',
            '--metadata=toc-title:目录', '--metadata=lang:en', '--embed-resources', '--css', str(ROOT / CSS),
            '--output', str(html),
        ], check=True)
        # Pandoc's installed locale catalog lacks zh; body language remains
        # Chinese and Noto CJK shaping is selected explicitly by the stylesheet.
        html.write_text(html.read_text().replace('lang="en"', 'lang="zh-CN"'))
        HTML(string=html.read_text(), base_url=html.as_uri(),
             url_fetcher=URLFetcher(allowed_protocols={'data'}, fail_on_errors=True)
             ).write_pdf(folder / 'manual.pdf')
        if before != sources():
            raise ValueError('Source changed during PDF rendering')
        shutil.copyfile(folder / 'manual.pdf', ROOT / PDF)
    report = {
        'schema': 1, 'purpose': 'ENGINEERING_BUILD_INSTRUCTIONS_ONLY',
        'production_release': False, 'hardware_manufactured_or_tested': False,
        'exported_at_utc': datetime.now(timezone.utc).isoformat(),
        'pandoc': subprocess.check_output(['pandoc', '--version'], text=True).splitlines()[0],
        'weasyprint': version('weasyprint'),
        'source_sha256': before, 'pdf_sha256': sha(PDF),
        'pdf_bytes': (ROOT / PDF).stat().st_size,
        'limits': 'Source/PDF consistency only; physical fit, assembly, electrical operation and security remain unqualified.'
    }
    (ROOT / MANIFEST).write_text(json.dumps(report, indent=2) + '\n')
    verify()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    try:
        verify() if args.check else render()
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
