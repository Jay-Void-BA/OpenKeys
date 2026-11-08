#!/usr/bin/env python3
"""Fail-closed handoff inventory. Never interprets file presence as certification.

Default output is manufacturing/readiness-report.json; exit 2 means blocked.
A complete file set is eligible for manual engineering/security review only.
No flashing, provisioning, network access, or private-key handling occurs here.
"""
from pathlib import Path
import csv,datetime,hashlib,json,re,sys,xml.etree.ElementTree as ET
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'manufacturing/readiness-report.json'
checks=[]
def check(name,ok,detail):checks.append({'name':name,'passed':bool(ok),'detail':detail})
def local(name):
 p=(ROOT/name).resolve()
 if not p.is_relative_to(ROOT):raise ValueError('Artifact must remain within project')
 return p
bom=list(csv.DictReader((ROOT/'hardware/r03/BOM.csv').open()))
unresolved=[r['ref'] for r in bom if 'PENDING' in r['footprint'] or 'NOT SELECTED' in r['value']]
check('schematic_footprint_selection',not unresolved,{'unresolved':unresolved,'bom_positions':len(bom)})
critical=list(csv.DictReader((ROOT/'manufacturing/critical-parts.csv').open()))
open_parts=[r['reference'] for r in critical if r['selection_status'] not in ('released','alternative_not_in_current_circuit')]
check('critical_parts_frozen_for_production',not open_parts,{'not_released':open_parts,'note':'EVT selection is not production approval'})
erc=json.loads((ROOT/'hardware/r03/review/erc.json').read_text())
viol=[v for s in erc['sheets'] for v in s['violations']]
check('erc_zero_errors_and_unresolved_warnings',not viol,{'errors':sum(v['severity']=='error' for v in viol),'warnings':sum(v['severity']=='warning' for v in viol)})
try:manifest=json.loads((ROOT/'manufacturing/release-manifest.json').read_text())
except (FileNotFoundError,json.JSONDecodeError):manifest={}
required={
 'board_source':'hardware/r03/kicad/OpenKeys.kicad_pcb',
 'drc_report':'manufacturing/evidence/pcb-drc.json',
 'fabrication_zip':'manufacturing/release/gerber-and-drill.zip',
 'assembly_drawing':'manufacturing/release/assembly.pdf',
 'placements':'manufacturing/release/placements.csv',
 'firmware_elf':'firmware/dist/openkeys.elf',
 'firmware_bin':'firmware/dist/openkeys.bin',
 'signed_update_bundle':'firmware/dist/openkeys-update.bin',
 'bootloader':'firmware/dist/bootloader.bin',
 'provisioning_instructions':'manufacturing/release/provisioning.md',
 'manager_macos':'dist/macos/AuthDesk.dmg',
 'manager_windows':'dist/windows/AuthDesk.msi',
 'manager_linux':'dist/linux/authdesk.tar.gz',
 'qualification_report':'manufacturing/evidence/qualification.json',
 'release_signoff':'manufacturing/evidence/release-signoff.json',
}
artifacts=manifest.get('artifacts',{})
check('release_manifest_present',bool(manifest),'Explicit artifact paths and SHA-256 hashes required')
for role,default in required.items():
 spec=artifacts.get(role,{})
 try:
  path=local(spec.get('path',default));exists=path.is_file();digest=hashlib.sha256(path.read_bytes()).hexdigest() if exists else None
  expected=spec.get('sha256','')
  ok=exists and re.fullmatch('[0-9a-f]{64}',expected or '') is not None and digest==expected
 except (OSError,ValueError,TypeError):exists=False;ok=False;digest=None
 check('artifact_'+role,ok,{'path':spec.get('path',default),'exists':exists,'hash_verified':bool(ok)})
# Sanity checks independently reject a floorplan or a host ELF passed as firmware.
if 'board_source' in artifacts:
 try:
  board=local(artifacts['board_source']['path']).read_text()
  fp_count=len(re.findall(r'\(footprint\s',board));track_count=len(re.findall(r'\(segment\s',board))
  minimum=sum(r['dnp']!='True' for r in bom)
  check('pcb_contains_complete_placement_and_tracks',fp_count>=minimum and track_count>0,{'footprints':fp_count,'minimum':minimum,'segments':track_count})
 except (KeyError,OSError,ValueError):check('pcb_contains_complete_placement_and_tracks',False,'Board source missing or invalid')
else:check('pcb_contains_complete_placement_and_tracks',False,'Only an explicitly unrouted floorplan is currently provided')
if 'firmware_elf' in artifacts:
 try:
  b=local(artifacts['firmware_elf']['path']).read_bytes();ok=len(b)>52 and b[:7]==b'\x7fELF\x01\x01\x01' and b[18:20]==b'\x28\x00'
  check('firmware_is_arm_elf',ok,'Requires 32-bit little-endian ARM ELF; not proof of firmware behavior')
 except (KeyError,OSError,ValueError):check('firmware_is_arm_elf',False,'Missing or invalid ARM firmware ELF')
else:check('firmware_is_arm_elf',False,'Device-policy is a library, not a flashable authenticator')
physical_tests=['fido2_u2f_hmac_secret','pin_and_biometric_persistent_retries','biometric_secure_pairing_and_replay','se_nonexport_and_object_policy','signed_boot_update_rollback_debug','vault_atomicity_and_rollback','battery_charge_suspend_and_backup_tamper','single_usb_c_current_esd_and_backfeed','usb_guard_ovlo_transients_thermal_and_reverse','nfc_final_enclosure_ios','trusted_screen_and_cancel','manager_three_os_real_device','emc_safe_failure','side_channel_and_fault_scope']
try:
 q=local(artifacts.get('qualification_report',{}).get('path',required['qualification_report']))
 qualification=json.loads(q.read_text());results=qualification.get('tests',{})
except (OSError,ValueError,TypeError):qualification={};results={}
missing_tests=[]
for name in physical_tests:
 r=results.get(name,{})
 try:
  ev=local(r['evidence_path']);h=hashlib.sha256(ev.read_bytes()).hexdigest()
  valid=r.get('result')=='pass' and h==r.get('evidence_sha256')
 except (KeyError,OSError,ValueError,TypeError):valid=False
 if not valid:missing_tests.append(name)
check('physical_qualification_complete',not missing_tests and bool(qualification.get('board_revision')) and bool(qualification.get('firmware_sha256')),{'missing':missing_tests,'note':'Evidence authenticity and scope require independent human review'})
complete=all(c['passed'] for c in checks)
report={'schema':1,'date_beijing':datetime.datetime.now(datetime.timezone(datetime.timedelta(hours=8))).isoformat(),'status':'ELIGIBLE_FOR_MANUAL_RELEASE_REVIEW' if complete else 'BLOCKED','manufacturing_release_authorized':False,'automated_tool_is_not_release_authority':True,'checks':checks,'blocker_count':sum(not c['passed'] for c in checks),'limits':'Checks inventory and declared hashes, not the truth of lab reports or security certification. Human signoff and actual qualification are mandatory.'}
OUT.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(report['status']+': '+str(report['blocker_count'])+' incomplete handoff conditions; see manufacturing/readiness-report.json')
for c in checks:
 if not c['passed']:print(' - '+c['name'])
sys.exit(0 if complete else 2)
