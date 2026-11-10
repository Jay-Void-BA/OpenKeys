#!/usr/bin/env python3
"""Reject stale R05 diagnostic sources, PCB contract, version or artifact bytes."""
import json
import struct
from r05_diagnostic_contract import ROOT, hardware, sha, sources


def main():
    dist = ROOT/'firmware/dist/r05'
    data = json.loads((dist/'diagnostic-manifest.json').read_text())
    assert data['schema']==1 and data['revision']=='0.5'
    assert data['purpose']=='R05_PUBLIC_ELECTRICAL_DIAGNOSTIC_ONLY'
    assert data['target']=='thumbv8m.main-none-eabihf'
    assert data['boot_profile']=='OPEN lifecycle, TZEN=0; NOT production STiRoT'
    for key in ['production_authenticator','signed_update','provisioning_or_flashing_performed',
                'charging_enabled','pin_verification','fido','vault','nfc_protocol',
                'usb_protocol','fingerprint_hardware']:
        assert data[key] is False, key
    assert data['se_disabled'] is True and data['physical_test_result']=='NOT_RUN'
    assert data['hardware_sha256']==hardware(), 'Stale R05 hardware binding'
    assert data['source_sha256']==sources(), 'Stale R05 diagnostic source inventory'
    assert len(data['images'])==2
    for image,(revision,address,features) in zip(data['images'],[
            ('v1-1',0x6b,'r05-diagnostics'),('v1-0',0x60,'r05-diagnostics,lcd-v1-0')]):
        assert (image['lcd_revision'],image['backlight_address'],image['features'])==(revision,address,features)
        names=[f'openkeys-r05-diagnostics-{revision}.{ext}' for ext in ['elf','bin']]
        assert image['artifacts']=={name:sha(dist/name) for name in names}
        elf,code=[(dist/name).read_bytes() for name in names]
        assert elf[:7]==b'\x7fELF\x01\x01\x01' and elf[18:20]==b'\x28\x00'
        assert 8<=len(code)<=2*1024*1024 and len(code)==image['binary_bytes']
        stack,reset=struct.unpack_from('<II',code)
        assert 0x20000000<stack<=0x20040000 and reset&1
        assert 0x08000000<=reset<0x08000000+len(code)
        assert (hex(stack),hex(reset))==(image['initial_stack'],image['reset_vector'])
    print('Verified both R05 diagnostic variants and source/PCB bindings; physical tests NOT_RUN')


if __name__=='__main__':
    main()
