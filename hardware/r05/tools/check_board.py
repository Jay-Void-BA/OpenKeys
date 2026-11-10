#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""Independent PCB-to-XML parity, stack/reference and manufacturing checks."""
from pathlib import Path
import collections, csv, hashlib, json, math, xml.etree.ElementTree as ET
import pcbnew as p
R=Path(__file__).resolve().parents[1]
path=R/'kicad/OpenKeys.kicad_pcb';b=p.LoadBoard(str(path))
x=ET.parse(R/'exports/OpenKeys-netlist.xml').getroot()
expected={(z.attrib['ref'],z.attrib['pin']):n.attrib['name'] for n in x.findall('./nets/net') for z in n.findall('node') if not n.attrib['name'].startswith('unconnected-')}
bom=list(csv.DictReader((R/'BOM.csv').open()));fps={f.GetReference():f for f in b.GetFootprints()}
checks=[]
def require(name,condition):
 if not condition:raise SystemExit('FAILED: '+name)
 checks.append(name)
require('Every schematic component is physically placed',all(c['ref'] in fps for c in bom))
observed={}
for c in bom:
 f=fps[c['ref']]
 require('Package '+c['ref'],f.GetFPID().GetLibItemName()==c['footprint'].split(':',1)[1])
 for pad in f.Pads():
  key=(c['ref'],pad.GetNumber());name=pad.GetNetname()
  require('Pad/net '+c['ref']+'.'+pad.GetNumber(),name==expected.get(key,''))
  if key in expected:observed[key]=name
require('No schematic pin omitted from PCB',observed==expected)
require('Four-layer1.0mm board',b.GetCopperLayerCount()==4 and b.GetDesignSettings().GetBoardThickness()==p.FromMM(1))
zones=list(b.Zones())
require('In1 and In2 contain only GND zones',len(zones)==2 and {z.GetLayer() for z in zones}=={p.In1_Cu,p.In2_Cu} and all(z.GetNetname()=='GND' and z.IsFilled() for z in zones))
for z in zones:
 require('Plane excludes rotated RF winding',all(not z.Outline().Contains(p.VECTOR2I(p.FromMM(100+xx),p.FromMM(80+yy))) for xx in [2,5,10,15,17] for yy in [46,60,80,100,108]))
tracks=list(b.GetTracks());segments=[t for t in tracks if t.GetClass()=='PCB_TRACK'];vias=[t for t in tracks if t.GetClass()=='PCB_VIA']
require('Electrical routing exists',len(segments)>500 and len(vias)>50)
require('Continuous In1 reference has no signal routes',not any(t.GetLayer()==p.In1_Cu and t.GetNetname()!='GND' for t in segments))
def outside_rf(t):
 a,c=t.GetStart(),t.GetEnd()
 length=(c-a).EuclideanNorm()/1e6
 for i in range(max(1,math.ceil(length/.25))+1):
  ratio=i/max(1,math.ceil(length/.25));xx=p.ToMM(a.x+(c.x-a.x)*ratio)-100;yy=p.ToMM(a.y+(c.y-a.y)*ratio)-80
  if xx<19 and yy>43:return False
 return True
require('Digital routes exclude the RF region',all(t.GetNetname() in ('ANT1','ANT2') or outside_rf(t) for t in segments))
require('No sub150um track necks',all(t.GetWidth()>=p.FromMM(.15) for t in segments))
require('Only through vias with300um drills',all(t.GetViaType()==p.VIATYPE_THROUGH and t.GetDrillValue()==p.FromMM(.3) for t in vias))
switch_lengths={name:sum(math.dist(p.ToMM(t.GetStart()),p.ToMM(t.GetEnd())) for t in segments if t.GetNetname()==name) for name in ('SW_L1','SW_L2','VCAP1','VCAP2')}
require('Short converter switch loops and VCAP tracks',all(0<value<6 for value in switch_lengths.values()))
require('Switch nodes remain on B.Cu with no vias',all(t.GetLayer()==p.B_Cu for t in segments if t.GetNetname() in ('SW_L1','SW_L2')) and not any(t.GetNetname() in ('SW_L1','SW_L2','VCAP1','VCAP2') for t in vias))
d=json.loads((R/'review/routed-drc.json').read_text())
require('KiCad DRC zero violations and zero unconnected items',not d['violations'] and not d['unconnected_items'] and not d['schematic_parity'])
require('NFC antenna is integral copper, excluded from bought parts',fps['J4'].IsNetTie() and bool(fps['J4'].GetAttributes() & p.FP_EXCLUDE_FROM_BOM))
keys=[(36,50),(54,50),(72,50),(36,64),(54,64),(72,64),(36,78),(54,78),(72,78),(54,92)]
require('Exact ten-key mechanical positions',all((round(p.ToMM(fps['SW'+str(i+2)].GetPosition().x)-100,4),round(p.ToMM(fps['SW'+str(i+2)].GetPosition().y)-80,4))==point for i,point in enumerate(keys)))
require('No fingerprint footprint placed',all(ref not in fps for ref in ['J5','U8','U9','Q3']))
require('No unreviewed DRC exclusions',not json.loads((R/'kicad/OpenKeys.kicad_pro').read_text())['board']['design_settings']['drc_exclusions'])
report={'revision':'0.5','board_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'checks':checks,'components':len(bom),'segments':len(segments),'vias':len(vias),'short_power_net_lengths_mm':switch_lengths,'drc_violations':0,'unconnected':0,'pcb_to_schematic_parity':True,'manufacturing_security_release':False,'physical_qualification':False}
(R/'review/board-check.json').write_text(json.dumps(report,indent=2)+'\n')
print(f'{len(checks)} PCB checks passed; {len(segments)} segments,{len(vias)} vias; no physical/security qualification inferred')
