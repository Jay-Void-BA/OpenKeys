#!/usr/bin/env python3
"""Build both explicit LCD diagnostic variants. No flashing or commissioning."""
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
from r05_diagnostic_contract import ROOT, hardware, sha, sources

TARGET = 'thumbv8m.main-none-eabihf'
TOOLCHAIN = '1.99.1'


def main():
    subprocess.run(['python3', 'scripts/freeze-r05-board.py', '--check'], cwd=ROOT, check=True)
    version = subprocess.check_output(['rustc', '+'+TOOLCHAIN, '-vV'], text=True)
    host = next(line[6:] for line in version.splitlines() if line.startswith('host: '))
    sysroot = Path(subprocess.check_output(['rustc', '+'+TOOLCHAIN, '--print', 'sysroot'], text=True).strip())
    objcopy = sysroot/'lib/rustlib'/host/'bin'/('rust-objcopy.exe' if os.name=='nt' else 'rust-objcopy')
    env = os.environ.copy()
    for name in ('LD_LIBRARY_PATH','DYLD_LIBRARY_PATH'):
        env[name] = str(sysroot/'lib')+(os.pathsep+env[name] if env.get(name) else '')
    build = Path(os.environ.get('CARGO_TARGET_DIR', ROOT/'target')).resolve()
    dist = ROOT/'firmware/dist/r05'
    dist.mkdir(exist_ok=True)
    images = []
    for revision, address, features in [('v1-1',0x6b,'r05-diagnostics'),
                                        ('v1-0',0x60,'r05-diagnostics,lcd-v1-0')]:
        subprocess.run(['cargo','+'+TOOLCHAIN,'build','-p','openkeys-h573-r05',
                        '--release','--target',TARGET,'--features',features,'--locked'], cwd=ROOT,check=True)
        elf = dist/f'openkeys-r05-diagnostics-{revision}.elf'
        binary = elf.with_suffix('.bin')
        shutil.copyfile(build/TARGET/'release/openkeys-r05-diagnostics', elf)
        subprocess.run([str(objcopy),'--output-target=binary',str(elf),str(binary)],env=env,check=True)
        raw, code = elf.read_bytes(), binary.read_bytes()
        assert raw[:7]==b'\x7fELF\x01\x01\x01' and raw[18:20]==b'\x28\x00'
        stack, reset = struct.unpack_from('<II',code)
        assert 8<=len(code)<=2*1024*1024
        assert 0x20000000<stack<=0x20040000 and reset&1
        assert 0x08000000<=reset<0x08000000+len(code)
        images.append(dict(lcd_revision=revision,backlight_address=address,
                           features=features,binary_bytes=len(code),initial_stack=hex(stack),
                           reset_vector=hex(reset),artifacts={p.name:sha(p) for p in [elf,binary]}))
    report = dict(schema=1,revision='0.5',purpose='R05_PUBLIC_ELECTRICAL_DIAGNOSTIC_ONLY',
                  target=TARGET,rustc=version.strip(),production_authenticator=False,
                  signed_update=False,provisioning_or_flashing_performed=False,
                  boot_profile='OPEN lifecycle, TZEN=0; NOT production STiRoT',
                  se_disabled=True,charging_enabled=False,pin_verification=False,
                  fido=False,vault=False,nfc_protocol=False,usb_protocol=False,
                  fingerprint_hardware=False,physical_test_result='NOT_RUN',
                  hardware_sha256=hardware(),source_sha256=sources(),images=images)
    (dist/'diagnostic-manifest.json').write_text(json.dumps(report,indent=2)+'\n')
    print('Exported two R05 ARM diagnostic variants, source-bound; not production and not physically tested')


if __name__=='__main__':
    main()
