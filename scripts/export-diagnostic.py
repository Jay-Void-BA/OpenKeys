#!/usr/bin/env python3
"""Build and bind the real R04 ARM diagnostic; never provisions or flashes."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]
TOOLCHAIN = '1.99.1'
TARGET = 'thumbv8m.main-none-eabihf'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    subprocess.run(['cargo', '+' + TOOLCHAIN, 'build', '-p', 'openkeys-h573', '--release',
                    '--target', TARGET, '--features', 'r04-diagnostics', '--locked'], cwd=ROOT, check=True)
    version = subprocess.check_output(['rustc', '+' + TOOLCHAIN, '-vV'], text=True)
    host = next(line.removeprefix('host: ') for line in version.splitlines() if line.startswith('host: '))
    sysroot = Path(subprocess.check_output(['rustc', '+' + TOOLCHAIN, '--print', 'sysroot'], text=True).strip())
    target_dir = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target')).resolve()
    elf = target_dir / TARGET / 'release/openkeys-r04-diagnostics'
    dist = ROOT / 'firmware/dist'
    dist.mkdir(exist_ok=True)
    output = dist / 'openkeys-r04-diagnostics.elf'
    shutil.copyfile(elf, output)
    objcopy = sysroot / 'lib/rustlib' / host / 'bin' / ('rust-objcopy.exe' if os.name == 'nt' else 'rust-objcopy')
    env = os.environ.copy()
    for name in ('LD_LIBRARY_PATH', 'DYLD_LIBRARY_PATH'):
        env[name] = str(sysroot / 'lib') + (os.pathsep + env[name] if env.get(name) else '')
    binary = output.with_suffix('.bin')
    subprocess.run([str(objcopy), '--output-target=binary', str(output), str(binary)], env=env, check=True)
    raw, code = output.read_bytes(), binary.read_bytes()
    assert raw[:7] == b'\x7fELF\x01\x01\x01' and raw[18:20] == b'\x28\x00', 'Not ARM32 little-endian ELF'
    stack, reset = struct.unpack_from('<II', code)
    assert 0 < len(code) <= 2 * 1024 * 1024
    assert 0x20000000 < stack <= 0x20040000 and reset & 1
    assert 0x08000000 <= reset < 0x08000000 + len(code)
    files = [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', Path(__file__).resolve()]
    for folder in ('firmware/board', 'firmware/target', 'firmware/vendor/cortex-m', 'crates/device-policy'):
        files.extend(p for p in (ROOT / folder).rglob('*') if p.is_file())
    report = {
        'schema': 1, 'purpose': 'R04_ELECTRICAL_DIAGNOSTIC_ONLY',
        'production_authenticator': False, 'signed_update': False,
        'provisioning_or_flashing_performed': False,
        'boot_profile': 'OPEN lifecycle, TZEN=0; NOT production STiRoT',
        'fido': False, 'vault': False, 'sensor_and_se_disabled': True,
        'charging_enabled': False, 'physical_test_result': 'NOT_RUN',
        'target': TARGET, 'rustc': version.strip(),
        'binary_bytes': len(code), 'initial_stack': hex(stack), 'reset_vector': hex(reset),
        'hardware_sha256': {
            'pcb': sha(ROOT / 'hardware/r04/kicad/OpenKeys.kicad_pcb'),
            'pinout': sha(ROOT / 'hardware/r04/pinout.csv'),
        },
        'source_sha256': {str(p.relative_to(ROOT)): sha(p) for p in sorted(set(files))},
        'artifacts': {p.name: sha(p) for p in (output, binary)},
    }
    (dist / 'diagnostic-manifest.json').write_text(json.dumps(report, indent=2) + '\n')
    print(f'Diagnostic only: {len(code)} bytes; ARM vectors and source hashes recorded; physical tests NOT_RUN')


if __name__ == '__main__':
    main()
