#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""Build the electrical PCB from the exported schematic, then import routing.

Run with KiCad's /usr/bin/python3. All coordinates below are PCB-local mm;
the KiCad sheet origin is (100,80). No net is invented by the layout script.
"""
from pathlib import Path
import argparse, csv, json, math, xml.etree.ElementTree as ET
import pcbnew as p

R=Path(__file__).resolve().parents[1]
CAD=R/'kicad'
ORIGIN=(100,80)
def v(x,y):return p.VECTOR2I(p.FromMM(x+ORIGIN[0]),p.FromMM(y+ORIGIN[1]))
def mm(a):return (round(p.ToMM(a.x)-100,5),round(p.ToMM(a.y)-80,5))
def edge(b, a,c):
 z=p.PCB_SHAPE();z.SetShape(p.SHAPE_T_SEGMENT);z.SetStart(v(*a));z.SetEnd(v(*c));z.SetLayer(p.Edge_Cuts);z.SetWidth(p.FromMM(.05));b.Add(z)

# Front: raised LCD, contextual switches and FP connector. Back: main digital
# electronics under the LCD. Cell clearance on B.Cu: x15..47,y33..73.
PLACEMENT={
 'J1':(34,.5,180,'F'), 'J2':(57,27,0,'B'), 'J3':(59,10,0,'B'),
 'J4':(3,72,0,'F'), 'J5':(20,48.3,90,'F'), 'J6':(61,43,180,'B'),
 'U1':(35,9,0,'B'), 'U2':(8,17,0,'B'), 'U3':(17,24,0,'B'),
 'U4':(36,21,0,'B'), 'U5':(54,38.5,0,'F'), 'U6':(57,61,0,'F'),
 'U7':(8,68,0,'F'), 'U8':(39.25,33.35,0,'F'), 'U9':(44,38,0,'F'),
 'U10':(49,16,0,'B'), 'U11':(50,23,0,'B'), 'U12':(49,29,0,'B'),
 'U13':(8,30,0,'B'), 'U14':(17,8,0,'B'),
 'U15':(51,38,0,'B'),
 'Q1':(45,9,0,'B'), 'Q2':(8,23,0,'B'), 'Q3':(62,39,0,'F'),
 'D1':(24,10,90,'B'), 'D2':(30,8,90,'F'), 'D3':(38,8,90,'F'),
 'D4':(13,8,90,'B'), 'D5':(22,5,90,'B'), 'F1':(26,5,0,'B'),
 'L1':(12.5,24,-90,'B'), 'Y1':(45,19.5,90,'B'),
 'SW1':(1,20,-90,'F'), 'SW2':(10,38.5,0,'F'), 'SW3':(23,38.5,0,'F'),
 'SW4':(10,59,0,'F'), 'SW5':(23,59,0,'F'), 'SW6':(6,53,0,'F'),
 'FB1':(26,13,90,'B'), 'FB2':(15,48,90,'F'),
 'R1':(29,6,90,'B'), 'R2':(39,6,90,'B'),
 'R3':(43,3,0,'B'), 'C1':(48,3,0,'B'),
 'R4':(45,12,0,'B'), 'R5':(48,6,0,'B'),
 'R6':(5,15,90,'B'), 'R7':(14,17.4,90,'B'),
 'R8':(10,14,90,'B'), 'R9':(6,24,90,'B'), 'R10':(22,20.5,90,'B'),
 'C2':(10,10,0,'B'), 'C3':(5,21,90,'B'), 'C4':(11,17,90,'B'),
 'C5':(16,20,-90,'B'), 'C6':(22,23,0,'B'),
 'C7':(21,25,90,'B'), 'C8':(24,25,90,'B'),
 'C14':(25,31,0,'B'), 'C18':(25,16,90,'B'),
 'R11':(45,32,0,'B'), 'C19':(45,29,0,'B'), 'R12':(42,32,90,'B'),
 'C20':(45,16.5,0,'B'), 'C21':(45,22.5,0,'B'),
 'R13':(51,35.5,0,'F'), 'R14':(54.5,35.25,0,'F'), 'R15':(58,37,90,'F'),
 'R16':(51,56,0,'F'), 'R17':(54,56,0,'F'), 'R18':(57,56,0,'F'),
 'R19':(60,17,0,'B'), 'R20':(60,19,0,'B'),
 'C22':(60,38,0,'B'), 'C23':(54,34.5,90,'B'), 'C24':(57,34.5,90,'B'),
 'C25':(62,61,90,'F'), 'C26':(5,66,90,'F'),
 'C27':(3,63,90,'F'), 'C28':(8,64.5,90,'B'),
 'C29':(5.5,70.5,0,'F'), 'R21':(16,70,0,'F'), 'R22':(16,65,0,'F'),
 'C30':(41,36,90,'F'), 'C31':(47,36,90,'F'),
 'C32':(36,36,0,'F'), 'C33':(33,36,0,'F'),
 'C34':(16,52,0,'F'), 'C35':(16,45,0,'F'),
 'R23':(38,38,0,'F'), 'R24':(38,40,0,'F'), 'R25':(42,41,0,'F'),
 'R26':(26,48,90,'F'), 'R27':(26,51,90,'F'), 'R28':(23,54,0,'F'),
 'R29':(62,34,0,'F'), 'R30':(62,42,0,'F'),
 'R31':(6,40,90,'F'), 'C36':(6,43,90,'F'),
 'R32':(27,40,90,'F'), 'C37':(27,43,90,'F'),
 'R33':(6,59,90,'F'), 'C38':(6,62,90,'F'),
 'R34':(27,59,90,'F'), 'C39':(27,62,90,'F'),
 'C40':(5,30,90,'B'), 'C41':(11,30,90,'B'),
 'R35':(4,49,90,'F'), 'R36':(52,28,90,'B'),
 'C42':(52,16,90,'B'), 'C43':(53,23,90,'B'), 'C44':(46,26,90,'B'),
 'R37':(14,5,0,'B'), 'R38':(13,11,0,'B'),
 'R39':(19,5,0,'B'), 'R40':(20,8,90,'B'),
 'R41':(19,11,0,'B'), 'R42':(23,14,0,'B'),
 'C45':(17,3,0,'B'), 'C46':(16,11,90,'B'),
 'R43':(60,46,0,'F'), 'C47':(51,41,0,'B'),
}

def courtyard(f):
 points=[]
 for z in f.GraphicalItems():
  if z.GetLayer() in (p.F_CrtYd,p.B_CrtYd):
   bb=z.GetBoundingBox();points.extend([mm(bb.GetPosition()),mm(bb.GetEnd())])
 if not points:
  bb=f.GetBoundingBox(False,False)
  return (*mm(bb.GetPosition()),*mm(bb.GetEnd()))
 return (min(t[0] for t in points),min(t[1] for t in points),max(t[0] for t in points),max(t[1] for t in points))

def overlap(a,b,gap=.08):
 return a[0]<b[2]+gap and b[0]<a[2]+gap and a[1]<b[3]+gap and b[1]<a[3]+gap

def pack_passives(b,fps):
 """Move only small passives to the nearest free 0.5mm site.

 Courts remain real library courtyards. This is placement assistance, not a
 DRC waiver: the final KiCad DRC is run independently after routing.
 """
 fixed=[f for f in b.GetFootprints() if f.GetReference().startswith(('U','J','SW','H','L','Y','Q','D','F'))]
 fixed=[f for f in fixed if f.GetReference() not in ('J4',) and not f.GetReference().startswith('FB')]
 movable=[f for f in fps.values() if f not in fixed and f.GetReference()!='J4']
 priority=['C15','C16','C9','C10','C11','C12','C13','C17','C18','C20','C21','C2','C3','C4','C5','C6','C7','C8','C45','C46']
 movable.sort(key=lambda f:priority.index(f.GetReference()) if f.GetReference() in priority else 100)
 placed=list(fixed)
 # FPC flex/raised sensor envelope and protected pouch clearance.
 forbidden={p.F_Cu:[(24.5,44,49.3,52.5),(42,41,57,56)],p.B_Cu:[(15,33.5,47,73)]}
 for f in movable:
  original=mm(f.GetPosition());side=f.GetLayer();found=False
  options=sorted([(dx*.5,dy*.5) for dx in range(-18,19) for dy in range(-18,19)],key=lambda z:z[0]*z[0]+z[1]*z[1])
  for dx,dy in options:
   f.SetPosition(v(original[0]+dx,original[1]+dy));a=courtyard(f)
   if a[0]<.65 or a[2]>67.35 or a[1]<.6 or a[3]>73.7:continue
   if any(overlap(a,z) for z in forbidden[side]):continue
   if any(overlap(a,courtyard(z)) for z in placed if z.GetLayer()==side or z.GetReference().startswith('H')):continue
   # Plated/NP holes extend through the board even when the component is on
   # the opposite side. Reserve the hole/pad envelope on both faces.
   blocked=False
   for z in fixed+[fps['J4']]:
    if z.GetLayer()==side and z.GetReference()!='J4':continue
    for pad in z.Pads():
     if pad.GetAttribute() not in (p.PAD_ATTRIB_PTH,p.PAD_ATTRIB_NPTH) and z.GetReference()!='J4':continue
     bb=pad.GetBoundingBox();q=(*mm(bb.GetPosition()),*mm(bb.GetEnd()))
     if overlap(a,q,.25):blocked=True;break
    if blocked:break
   if blocked:continue
   found=True;placed.append(f);break
  if not found:raise RuntimeError('No legal passive location: '+f.GetReference())

def build():
 b=p.BOARD();ds=b.GetDesignSettings();ds.SetCopperLayerCount(4);ds.SetBoardThickness(p.FromMM(1))
 ds.SetAuxOrigin(v(0,92))
 ds.m_MinClearance=p.FromMM(.15);ds.m_TrackMinWidth=p.FromMM(.15);ds.m_ViasMinSize=p.FromMM(.6)
 ds.m_HoleClearance=p.FromMM(.25);ds.m_CopperEdgeClearance=p.FromMM(.3)
 nc=ds.m_NetSettings.GetDefaultNetclass()
 nc.SetClearance(p.FromMM(.15));nc.SetTrackWidth(p.FromMM(.15));nc.SetViaDiameter(p.FromMM(.6));nc.SetViaDrill(p.FromMM(.3))
 polygon=[(0,0),(28,0),(28,-2.5),(40,-2.5),(40,0),(68,0),(68,92),(0,92),(0,0)]
 for a,c in zip(polygon,polygon[1:]):edge(b,a,c)
 x=ET.parse(R/'exports/OpenKeys-netlist.xml').getroot()
 nets={};pin_net={}
 for n in x.findall('./nets/net'):
  name=n.attrib['name']
  if name.startswith('unconnected-'):continue
  z=p.NETINFO_ITEM(b,name);b.Add(z);nets[name]=z
  for node in n.findall('node'):pin_net[(node.attrib['ref'],node.attrib['pin'])]=name
 fps={}
 comps=list(csv.DictReader((R/'BOM.csv').open()))
 for c in comps:
  ref=c['ref'];lib,name=c['footprint'].split(':',1)
  path=str(CAD/'OpenKeys.pretty') if lib=='OpenKeys' else '/usr/share/kicad/footprints/'+lib+'.pretty'
  f=p.FootprintLoad(path,name)
  if not f:raise ValueError((ref,c['footprint']))
  f.SetReference(ref);f.SetValue(c['value']);f.SetDNP(c['dnp']=='True');b.Add(f);fps[ref]=f
  for pad in f.Pads():
   name=pin_net.get((ref,pad.GetNumber()))
   if name:pad.SetNet(nets[name])
  f.Reference().SetVisible(False);f.Value().SetVisible(False)
 for i,(x,y) in enumerate([(3,3),(65,3),(3,67),(65,67)],1):
  f=p.FootprintLoad('/usr/share/kicad/footprints/MountingHole.pretty','MountingHole_2.2mm_M2')
  f.SetReference('H'+str(i));f.SetPosition(v(x,y));f.Reference().SetVisible(False);f.Value().SetVisible(False);b.Add(f)
 def place(ref,item):
  xx,yy,angle,side=item;f=fps[ref]
  if side=='B' and f.GetLayer()==p.F_Cu:f.Flip(f.GetPosition(),p.FLIP_DIRECTION_LEFT_RIGHT)
  f.SetOrientationDegrees(angle);f.SetPosition(v(xx,yy))
 for ref,item in PLACEMENT.items():place(ref,item)
 # Each decoupler is tied to the actual power-pin coordinate, including the
 # two separate VCAP pins. Offset away from the package, never below it.
 decouplers={'C9':(19,0),'C10':(32,0),'C11':(48,0),'C12':(64,0),'C13':(64,2.3),'C15':(30,0),'C16':(62,0),'C17':(13,0)}
 for ref,(number,tangent) in decouplers.items():
  pad=next(z for z in fps['U4'].Pads() if z.GetNumber()==str(number));xx,yy=mm(pad.GetPosition())
  dx,dy=xx-36,yy-21
  if abs(dx)>abs(dy):xx+=math.copysign(3.5,dx);yy+=tangent;angle=0
  else:yy+=math.copysign(3.5,dy);xx+=tangent;angle=90
  place(ref,(xx,yy,angle,'B'))
 assert set(fps)==set(PLACEMENT)|set(decouplers),(set(fps)-set(PLACEMENT)-set(decouplers))
 # CC protection sits beside the connector on the reverse side, outside the
 # raised LCD's front-side envelope.
 for ref in ('D2','D3'):place(ref,(*PLACEMENT[ref][:3],'B'))
 pack_passives(b,fps)
 # Orient each nonpolar decoupler so its power land is nearest its actual
 # MCU pin. Avoid routing VCAP past the ground land on the reverse side.
 for ref,(number,_) in decouplers.items():
  f=fps[ref];target=next(z.GetPosition() for z in fps['U4'].Pads() if z.GetNumber()==str(number))
  positive=next(z for z in f.Pads() if z.GetNumber()=='1')
  before=(positive.GetPosition()-target).EuclideanNorm();angle=f.GetOrientationDegrees()
  f.SetOrientationDegrees(angle+180)
  if (positive.GetPosition()-target).EuclideanNorm()>=before:f.SetOrientationDegrees(angle)
 # The connector shell overhangs the USB tongue. Clip only its assembly
 # silkscreen to the actual tongue, retaining all copper and mounting slots.
 for z in fps['J1'].GraphicalItems():
  if z.GetLayer()==p.F_SilkS:
   a=mm(z.GetStart());c=mm(z.GetEnd())
   z.SetStart(v(a[0],max(-2.15,a[1])));z.SetEnd(v(c[0],max(-2.15,c[1])))
 # In1 is a continuous ground reference. In2 is ground fill around signal
 # routing; digital routes never cross In1. Both stop before the antenna.
 for layer in (p.In1_Cu,p.In2_Cu):
  z=p.ZONE(b);z.SetLayer(layer);z.SetNet(nets['GND']);z.SetLocalClearance(p.FromMM(.2));z.SetMinThickness(p.FromMM(.15));z.SetPadConnection(p.ZONE_CONNECTION_FULL)
  z.Outline().NewOutline()
  for xx,yy in [(.3,.3),(28.3,.3),(28.3,-2.2),(39.7,-2.2),(39.7,.3),(67.7,.3),(67.7,73),(.3,73)]:
   q=v(xx,yy);z.Outline().Append(q.x,q.y)
  b.Add(z)
 # Fix the high-di/dt inductor connections before general autorouting.
 # The inductor is immediately beside the converter, not below the VOUT
 # pin row. No switch node changes layer or enters the NFC region.
 def trace(net,points,width,layer=p.B_Cu):
  for a,c in zip(points,points[1:]):
   if abs(a[0]-c[0])+abs(a[1]-c[1])<.00001:continue
   z=p.PCB_TRACK(b);z.SetStart(v(*a));z.SetEnd(v(*c));z.SetWidth(p.FromMM(width));z.SetLayer(layer);z.SetNet(nets[net]);b.Add(z)
 def pin(ref,number):return mm(next(z.GetPosition() for z in fps[ref].Pads() if z.GetNumber()==str(number)))
 def via(net,point):
  z=p.PCB_VIA(b);z.SetPosition(v(*point));z.SetViaType(p.VIATYPE_THROUGH);z.SetLayerPair(p.F_Cu,p.B_Cu);z.SetWidth(p.F_Cu,p.FromMM(.6));z.SetDrill(p.FromMM(.3));z.SetNet(nets[net]);b.Add(z)
 trace('SW_L1',[(15.7875,23.5),(14.8,23.5)],.2)
 trace('SW_L1',[(14.8,23.5),(14.15,22.85),(12.5,22.85)],.35)
 trace('SW_L2',[(15.7875,24.5),(14.8,24.5)],.2)
 trace('SW_L2',[(14.8,24.5),(14.15,25.15),(12.5,25.15)],.35)
 for ref,number in [('C15','30'),('C16','62')]:
  mcu_pin=next(z for z in fps['U4'].Pads() if z.GetNumber()==number)
  cap=next(z for z in fps[ref].Pads() if z.GetNumber()=='1')
  a=mm(mcu_pin.GetPosition());c=mm(cap.GetPosition())
  mid=(c[0],a[1]) if abs(a[0]-36)>abs(a[1]-21) else (a[0],c[1])
  trace(mcu_pin.GetNetname(),[a,mid,c],.25)
 # Reserve normal exits from fine-pitch pads BEFORE GND fanout/general
 # routing can surround them. These are electrical tracks, not exclusions.
 iset=pin('R7',1)
 trace('CHG_ISET',[pin('U2',8),(12.5,17),(12.5+iset[1]-17,iset[1]),iset],.15)
 vina=pin('C6',1);r_vina=pin('R10',2)
 trace('REG_VINA',[pin('U3',8),(19.2,24),(20.2,23),vina],.15)
 trace('REG_VINA',[vina,(vina[0],r_vina[1]),r_vina],.15)
 for n,site in [(18,(54,36.2)),(15,(56.8,37.7))]:
  via('SE_CORE',site);trace('SE_CORE',[pin('U5',n),site],.15,p.F_Cu)
 trace('SE_CORE',[(54,36.2),(55.3,36.2),(56.8,37.7)],.2)
 trace('SE_CORE',[pin('C23',1),(54,36.2)],.2)
 trace('SE_CORE',[pin('C24',1),(57,37.5),(56.8,37.7)],.2)
 via('+3V3',(56.8,38.9));trace('+3V3',[pin('U5',12),(56.8,38.9)],.25,p.F_Cu)
 trace('+3V3',[(56.8,38.9),pin('C22',1)],.25)
 via('NFC_VOUT',(5.4,67.5));trace('NFC_VOUT',[pin('U7',2),(5.4,67.5)],.15,p.F_Cu)
 nfc_cap=pin('C28',1)
 trace('NFC_VOUT',[(5.4,67.5),(5.4,66.875),(7,nfc_cap[1]),nfc_cap],.15)
 # Electrical feed guards expose the in-core part of the fixed antenna to
 # the router. Removing the RF footprint graphics must not make these
 # conductors invisible to other-net clearance checking.
 trace('ANT1',[(3,72),(3,73.2)],.5,p.F_Cu)
 trace('ANT2',[(8,72),(7.581818,73.2)],.5)
 escapes=[('NFC_CS_N',6,[(9.4,69),(9.7,69),(10.5,69.8)]),('NFC_SCK',7,[(9.4,68.5),(11.7,68.5),(11.8,68.6),(12.7,68.6)]),('NFC_MOSI',8,[(9.4,68),(14,68)]),('NFC_MISO',9,[(9.4,67.5),(10.4,67.5),(10.5,67.4),(11.4,67.4)]),('NFC_IRQ_N',10,[(9.4,67),(9.5,67),(10.3,66.2)])]
 for net,n,path in escapes:
  assert math.dist(pin('U7',n),path[0])<.001
  trace(net,path,.15,p.F_Cu);via(net,path[-1])
 for n,site in [('A9',(31.55,6)),('A4',(36.45,6))]:
  trace('VBUS_RAW',[pin('J1',n),site],.3,p.F_Cu);via('VBUS_RAW',site)
 trace('VBUS_RAW',[(31.55,6),(36.45,6)],.3)
 via('VBUS_RAW',(24.6,3.5))
 trace('VBUS_RAW',[(24.6,3.5),pin('F1',1)],.3)
 trace('VBUS_RAW',[(31.55,6),(30.55,5),(26.1,5),(24.6,3.5)],.3,p.In2_Cu)
 # Explicit manufacturing rules, used by KiCad and Specctra export.
 project=json.loads((CAD/'OpenKeys.kicad_pro').read_text())
 project['board']={'design_settings':{'rules':{'min_clearance':.15,'min_track_width':.15,'min_via_diameter':.6,'min_via_annular_width':.15,'min_through_hole_diameter':.3,'min_hole_clearance':.25,'min_copper_edge_clearance':.3,'solder_mask_to_copper_clearance':0.0}}}
 project['net_settings']={'meta':{'version':4},'classes':[{'name':'Default','clearance':.15,'track_width':.2,'via_diameter':.6,'via_drill':.3,'diff_pair_width':.15,'diff_pair_gap':.15,'diff_pair_via_gap':.25,'bus_width':12,'wire_width':6}], 'netclass_patterns':[]}
 (CAD/'OpenKeys.kicad_pro').write_text(json.dumps(project,indent=2)+'\n')
 p.SaveBoard(str(CAD/'OpenKeys.kicad_pcb'),b)
 # Re-load with project rules before DSN generation.
 b=p.LoadBoard(str(CAD/'OpenKeys.kicad_pcb'))
 export_core()
 write_placement(b)
 print(f'Placed all {len(comps)} schematic components with {len(nets)} real nets')

def export_core():
 # Route only the digital/power core; the antenna has its own fixed copper.
 # A shortened routing-copy outline prevents any digital route in the RF area.
 router=p.LoadBoard(str(CAD/'OpenKeys.kicad_pcb'))
 for z in list(router.GetDrawings()):
  if z.GetLayer()==p.Edge_Cuts:router.RemoveNative(z)
 for z in list(router.GetTracks()):
  if z.GetNetname()=='ANT2' and (mm(z.GetStart())[1]>73.8 or mm(z.GetEnd())[1]>73.8):router.RemoveNative(z)
 for z in list(router.Zones()):
  if z.GetLayer()==p.In2_Cu:router.RemoveNative(z)
 polygon=[(0,0),(28,0),(28,-2.5),(40,-2.5),(40,0),(68,0),(68,92),(0,92),(0,0)]
 core=polygon.copy();core=[(xx,73.8 if yy==92 else yy) for xx,yy in core]
 for a,c in zip(core,core[1:]):edge(router,a,c)
 rf=next(f for f in router.GetFootprints() if f.GetReference()=='J4')
 for z in list(rf.GraphicalItems()):
  if z.GetLayer() in (p.F_Cu,p.B_Cu):rf.RemoveNative(z)
 for z in list(rf.Pads()):
  if mm(z.GetPosition())[1]>73.8:rf.RemoveNative(z)
  elif z.GetNumber()=='1':
   z.SetPosition(v(3,72.75));z.SetSize(p.VECTOR2I(p.FromMM(.5),p.FromMM(1.5)))
 if not p.ExportSpecctraDSN(router,str(R/'review/OpenKeys-core.dsn')):raise RuntimeError('Specctra export failed')
 dsn=R/'review/OpenKeys-core.dsn'
 text=dsn.read_text()
 for layer in ('In1.Cu',):
  text=text.replace('(layer '+layer+'\n      (type signal)', '(layer '+layer+'\n      (type power)')
 # Read settings after layers, before plane: v1.9 parser initializes layer arrays here.
 settings='    (autoroute_settings\n      (fanout off) (autoroute on) (postroute off) (vias on)\n      (via_costs 50) (plane_via_costs 5) (start_ripup_costs 100) (start_pass_no 1)\n      (layer_rule F.Cu (active on) (preferred_direction vertical)\n        (preferred_direction_trace_costs 1.6) (against_preferred_direction_trace_costs 2.7))\n      (layer_rule In1.Cu (active off) (preferred_direction vertical)\n        (preferred_direction_trace_costs 1) (against_preferred_direction_trace_costs 2.1))\n      (layer_rule In2.Cu (active on) (preferred_direction horizontal)\n        (preferred_direction_trace_costs 1) (against_preferred_direction_trace_costs 1.9))\n      (layer_rule B.Cu (active on) (preferred_direction vertical)\n        (preferred_direction_trace_costs 1.6) (against_preferred_direction_trace_costs 2.7))\n    )\n'
 text=text.replace('    (boundary\n',settings+'    (boundary\n',1)
 dsn.write_text(text)

def write_placement(b):
 with (R/'review/placement.csv').open('w') as out:
  w=csv.writer(out,lineterminator='\n');w.writerow(['ref','x_mm','y_mm','rotation_deg','side','dnp'])
  for f in sorted(b.GetFootprints(),key=lambda f:f.GetReference()):
   w.writerow([f.GetReference(),*mm(f.GetPosition()),round(f.GetOrientationDegrees(),3),'F' if f.GetLayer()==p.F_Cu else 'B',f.IsDNP()])

def import_routes(path):
 b=p.LoadBoard(str(CAD/'OpenKeys.kicad_pcb'))
 if not p.ImportSpecctraSES(b,str(path)):raise RuntimeError('Session import failed')
 # The RF underpass lies outside the shortened router boundary and is
 # restored as a real electrical track, not only a footprint drawing.
 rf=next(f for f in b.GetFootprints() if f.GetReference()=='J4')
 ends=[z.GetPosition() for z in rf.Pads() if z.GetNumber()=='2']
 for t in list(b.GetTracks()):
  if t.GetNetname()=='ANT2' and t.GetClass()=='PCB_TRACK' and t.GetLayer()==p.B_Cu and min(mm(t.GetStart())[1],mm(t.GetEnd())[1])>=71.9:b.RemoveNative(t)
 if not any(t.GetNetname()=='ANT2' and mm(t.GetStart())[1]>73.8 or t.GetNetname()=='ANT2' and mm(t.GetEnd())[1]>73.8 for t in b.GetTracks()):
  z=p.PCB_TRACK(b);z.SetStart(ends[0]);z.SetEnd(ends[1]);z.SetWidth(p.FromMM(.5));z.SetLayer(p.B_Cu);z.SetNet(b.FindNet('ANT2'));b.Add(z)
 if not p.ZONE_FILLER(b).Fill(b.Zones()):raise RuntimeError('Zone fill failed')
 p.SaveBoard(str(CAD/'OpenKeys.kicad_pcb'),b)
 print(f'Imported {len(b.GetTracks())} track/via objects')

if __name__=='__main__':
 a=argparse.ArgumentParser();a.add_argument('--import-ses',type=Path);a.add_argument('--export-dsn',action='store_true');args=a.parse_args()
 if args.import_ses:import_routes(args.import_ses)
 elif args.export_dsn:export_core()
 else:build()
