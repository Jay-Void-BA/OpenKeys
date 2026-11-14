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
 require('Saved value '+c['ref'],f.GetValue()==c['value'])
 for pad in f.Pads():
  key=(c['ref'],pad.GetNumber());name=pad.GetNetname()
  require('Pad/net '+c['ref']+'.'+pad.GetNumber(),name==expected.get(key,''))
  if key in expected:observed[key]=name
require('No schematic pin omitted from PCB',observed==expected)
require('Four-layer1.0mm board',b.GetCopperLayerCount()==4 and b.GetDesignSettings().GetBoardThickness()==p.FromMM(1))
def pad_position(ref,number):
 return next(pad.GetPosition() for pad in fps[ref].Pads() if pad.GetNumber()==str(number))
def pin_distance(first,second):
 return p.ToMM((pad_position(*first)-pad_position(*second)).EuclideanNorm())
require('Backup reverse blocker input decoupling within 3mm of VIN',pin_distance(('U13',1),('C40',1))<=3)
require('Backup bulk capacitance within 3.5mm of MCU VBAT',pin_distance(('U4',1),('C41',1))<=3.5)
require('MCU VBAT has separate decoupling within 3mm',pin_distance(('U4',1),('C56',1))<=3)
require('Backup decouplers use the component copper side',all(fps[cap].GetLayer()==fps[owner].GetLayer() for cap,owner in [('C40','U13'),('C41','U4'),('C56','U4')]))
require('Independent backup cell limiter is local to positive terminal',pin_distance(('BT1',1),('R61',1))<5 and fps['R61'].GetLayer()==fps['BT1'].GetLayer())
require('Active output damping and return bias are local to MCU pins',pin_distance(('U4',15),('R63',1))<=3.5 and pin_distance(('U4',2),('R62',1))<=3)
require('Active return RF shunt is local to backup-capable PC13',pin_distance(('U4',2),('C68',1))<=3)
require('Main-cell ADC output bias is local to PA0',pin_distance(('U4',14),('R68',1))<=3)
require('Main-cell reservoir is local to powered-off switch input',pin_distance(('U28',3),('C61',1))<=3.5)
require('Main-cell isolation switch is locally decoupled',pin_distance(('U28',2),('C67',1))<=3 and fps['C67'].GetLayer()==fps['U28'].GetLayer())
for cap,owner,pin in [('C63','U24',8),('C64','U25',5),('C65','U26',5),('C66','U27',5)]:
 require('Local mismatch detector decoupling '+cap,pin_distance((cap,1),(owner,pin))<=3 and fps[cap].GetLayer()==fps[owner].GetLayer())
for cap,owner,pin in [('C49','U18',1),('C55','U21',1),('C57','U21',8),('C58','U23',5),('C59','U22',5),('C60','U17',6)]:
 require('Local display/window decoupling '+cap,pin_distance((cap,1),(owner,pin))<=3 and fps[cap].GetLayer()==fps[owner].GetLayer())
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
for net in ['SYS_UV_SENSE','SYS_OV_SENSE']:
 items=[t for t in segments if t.GetNetname()==net]
 require('Short same-side comparator ladder '+net,bool(items) and sum(p.ToMM(t.GetLength()) for t in items)<10 and all(t.GetLayer()==p.B_Cu for t in items) and not any(t.GetNetname()==net for t in vias))
switch_lengths={name:sum(math.dist(p.ToMM(t.GetStart()),p.ToMM(t.GetEnd())) for t in segments if t.GetNetname()==name) for name in ('SW_L1','SW_L2','VCAP1','VCAP2')}
require('Short converter switch loops and VCAP tracks',all(0<value<6 for value in switch_lengths.values()))
require('Switch nodes remain on B.Cu with no vias',all(t.GetLayer()==p.B_Cu for t in segments if t.GetNetname() in ('SW_L1','SW_L2')) and not any(t.GetNetname() in ('SW_L1','SW_L2','VCAP1','VCAP2') for t in vias))
d=json.loads((R/'review/routed-drc.json').read_text())
require('KiCad DRC zero violations and zero unconnected items',not d['violations'] and not d['unconnected_items'] and not d['schematic_parity'])
require('NFC antenna is integral copper, excluded from bought parts',fps['J4'].IsNetTie() and bool(fps['J4'].GetAttributes() & p.FP_EXCLUDE_FROM_BOM))
keys=[(36,50),(54,50),(72,50),(36,64),(54,64),(72,64),(36,78),(54,78),(72,78),(54,92)]
require('Exact ten-key mechanical positions',all((round(p.ToMM(fps['SW'+str(i+2)].GetPosition().x)-100,4),round(p.ToMM(fps['SW'+str(i+2)].GetPosition().y)-80,4))==point for i,point in enumerate(keys)))
require('No fingerprint footprint placed',all(ref not in fps for ref in ['J5','U8','U9','Q3']))
def local_position(ref):
 a=fps[ref].GetPosition();return (p.ToMM(a.x)-100,p.ToMM(a.y)-80)
lid=[local_position(ref) for ref in ['SW13','SW14','SW15']]
area=abs(sum(a[0]*c[1]-c[0]*a[1] for a,c in zip(lid,lid[1:]+lid[:1])))/2
require('Three active lid contacts are dispersed and noncollinear',area>500 and all(math.dist(a,c)>30 for i,a in enumerate(lid) for c in lid[i+1:]))
require('Main reset and active-loop contacts retain exact enclosure coordinates',all(local_position(ref)==point for ref,point in [('SW12',(86,102.5)),('SW13',(81.25,49)),('SW14',(33,100)),('SW15',(77,105))]))
require('Primary backup cell uses exact qualified-layout footprint orientation',local_position('BT1')==(71.75,100) and fps['BT1'].GetLayer()==p.B_Cu and fps['BT1'].GetOrientationDegrees()==0)
# The KiCad HFN land pattern places the 20mm body centre +10.25mm from
# positive terminal 1 after this backside placement. Include 0.25mm margin.
coin=(local_position('BT1')[0]+10.25,local_position('BT1')[1]);radius=10.25
require('Primary coin body is inside board away from RF and pouch keepouts',coin[0]-radius>68 and coin[0]+radius<95.35 and coin[1]-radius>43 and coin[1]+radius<111.35)
require('No unreviewed DRC exclusions',not json.loads((R/'kicad/OpenKeys.kicad_pro').read_text())['board']['design_settings']['drc_exclusions'])
report={'revision':'0.5','board_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'checks':checks,'components':len(bom),'segments':len(segments),'vias':len(vias),'short_power_net_lengths_mm':switch_lengths,'drc_violations':0,'unconnected':0,'pcb_to_schematic_parity':True,'manufacturing_security_release':False,'physical_qualification':False}
(R/'review/board-check.json').write_text(json.dumps(report,indent=2)+'\n')
print(f'{len(checks)} PCB checks passed; {len(segments)} segments,{len(vias)} vias; no physical/security qualification inferred')
