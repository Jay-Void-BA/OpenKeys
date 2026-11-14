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


# Independent ten-key contacts; no fingerprint connector or sensor rails.
KEYS={str(i+2):(x,y) for i,(x,y) in enumerate([(36,50),(54,50),(72,50),(36,64),(54,64),(72,64),(36,78),(54,78),(72,78),(54,92)])}
PLACEMENT={
 'J1':(48,.5,180,'F'),'J2':(81,45,0,'B'),'J3':(90,9,0,'B'),
 'J4':(20,46,-90,'F'),'J6':(83.0,74.75,90,'B'),'J7':(90,43,0,'F'),
 'U1':(49,9,0,'B'),'U2':(8,17,0,'B'),'U3':(20,25,0,'B'),
 'U4':(50,24,0,'B'),'U5':(84,55,0,'F'),'U6':(84,81,0,'F'),
 'U7':(25,51,-90,'F'),'U10':(65,16,0,'B'),'U11':(72,18,0,'B'),
 'U12':(71,26,0,'B'),'U13':(41.75,34.75,0,'B'),'U14':(21,8,0,'B'),
 'U15':(76,31,0,'B'),'U16':(83,20,0,'B'),'U17':(76.25,23,0,'B'),
 'U18':(83,28,0,'B'),'U19':(82,35,0,'B'),'U20':(90,28,0,'B'),'U21':(91,20,0,'B'),
 'Q1':(59,9,0,'B'),'Q2':(8,24,0,'B'),
 'D1':(36,10,90,'B'),'D2':(44,8,90,'B'),'D3':(52,8,90,'B'),
 'D4':(16,8,90,'B'),'D5':(26,5,90,'B'),'F1':(34,5,0,'B'),
 'L1':(15.5,25,-90,'B'),'Y1':(60,22.5,90,'B'),'SW1':(95,65,90,'F'),
 'SW12':(86,102.5,0,'F'),'FB1':(40,16,90,'B'),
 'SW13':(81.25,49,0,'F'),'SW14':(33,100,0,'F'),'SW15':(77,105,0,'F'),
 'BT1':(71.75,100,0,'B'),
 'C29':(29,47,0,'F'),'R21':(29,54,0,'F'),
 'R37':(25,10,0,'B'),'R1':(43,12,0,'B'),'R2':(46,12.5,0,'B'),
 # Independent primary backup through a reverse-blocking ideal diode.
 'C40':(38.25,35.25,270.0,'B'),'C41':(42.0,31.25,270.0,'B'),'C56':(41.5,28.0,180.0,'B'),
 'C32':(76.0,59.0,270.0,'F'),
 'R43':(86.5,28,0,'B'),
 'C49':(79.5,28.5,180,'B'),
 'R12':(38.75,30,0,'B'),
 'U22':(25,38,0,'B'),'U23':(91,36,0,'B'),
 'R49':(23,34.5,90,'B'),'R50':(21,37,180,'B'),
 'R51':(27,34.5,90,'B'),'R52':(29,37,0,'B'),
 'R53':(91,39,0,'B'),'R54':(93.5,41,0,'B'),
 'R55':(88,16,0,'B'),'R56':(92,16,0,'B'),
 'C55':(89.0,23.5,270,'B'),'C57':(93.25,23.5,270,'B'),
 'C58':(93.5,36.0,90,'B'),'C59':(28.75,38.75,0,'B'),
 'R47':(88,41,0,'B'),'R48':(87.5,35.75,180,'B'),
 'R57':(77.75,25,0,'F'),'R58':(75.25,20.25,0,'B'),
 'R59':(86,17.75,0,'B'),'C60':(79.25,23,90,'B'),
 'R61':(68.75,98,180,'B'),'R62':(42.5,28.25,0,'F'),
 'R63':(46.5, 17.25, 90, 'F'),
 'R64':(34.5,22.5,90,'F'),'R65':(34.0,25.25,0,'F'),'C61':(36.75,28.75,0,'F'),
 'U24':(52,103,0,'F'),'U25':(60,103,0,'F'),'U26':(68,103,0,'F'),
 'R66':(56,106,0,'F'),'C62':(59.5,106,0,'F'),'R67':(44.5,14.75,0,'F'),
 'C63':(56,102.25,0,'F'),'C64':(63.5,102.35,0,'F'),'C65':(68.75,100.25,0,'F'),
 'U27':(45,103,0,'F'),'C66':(46.5,100.5,0,'F'),
 'U28':(37.25,26.25,270.0,'F'),'R68':(42.75,23.75,180,'F'),'C67':(36.5,22.25,90,'F'),
 'R69':(40,104,90,'F'),
 'C68':(46.0,30.25,270.0,'F'),
}
PLACEMENT.update({'SW'+ref:(x,y,0,'F') for ref,(x,y) in KEYS.items()})
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
 fixed=[f for f in b.GetFootprints() if f.GetReference().startswith(('U','J','SW','H','L','Y','Q','D','F','BT')) or f.GetReference() in ('C29','R21','R37','R1','R2','R12','C40','C41','C49','C55','C56','C57','C58','C59','C60','C61','R43','R47','R48','R49','R50','R51','R52','R53','R54','R55','R56','R57','R58','R59','R61','R62','R63','R64','R65','R66','R67','C62','C63','C64','C65','C66','C67','C68','R68','R69')]
 fixed=[f for f in fixed if f.GetReference() not in ('J4',) and not f.GetReference().startswith('FB')]
 movable=[f for f in fps.values() if f not in fixed and f.GetReference()!='J4']
 priority=['C15','C16','C9','C10','C11','C12','C13','C17','C18','C20','C21','C2','C3','C4','C5','C6','C7','C8','C45','C46']
 movable.sort(key=lambda f:priority.index(f.GetReference()) if f.GetReference() in priority else 100)
 placed=list(fixed)
 # RF winding and protected pouch clearance.
 forbidden={p.F_Cu:[(0,43,19,112),(23.3,46.9,26.7,49),(24.4,53.1,25.6,54.7),(83.5,51.5,84.5,53.3),(85.8,53.7,87.5,54.7)],p.B_Cu:[(0,43,19,112),(33,47,68,90),(21.9,7.9,23.8,8.6),(21.5,24.5,23.5,25.5),(45.1,4.4,46,6.5),(50,4.4,50.9,6.5),(55.7,21.8,57.8,22.7),(49.2,29.7,50.3,31.4)]}
 for f in movable:
  original=mm(f.GetPosition());side=f.GetLayer();found=False
  options=sorted([(dx*.5,dy*.5) for dx in range(-18,19) for dy in range(-18,19)],key=lambda z:z[0]*z[0]+z[1]*z[1])
  for dx,dy in options:
   f.SetPosition(v(original[0]+dx,original[1]+dy));a=courtyard(f)
   if a[0]<.65 or a[2]>95.35 or a[1]<.6 or a[3]>111.35:continue
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
 b=p.BOARD();ds=b.GetDesignSettings();ds.SetCopperLayerCount(4);ds.SetBoardThickness(p.FromMM(1));ds.SetAuxOrigin(v(0,112))
 ds.m_MinClearance=p.FromMM(.15);ds.m_TrackMinWidth=p.FromMM(.15);ds.m_ViasMinSize=p.FromMM(.6)
 ds.m_HoleClearance=p.FromMM(.25);ds.m_CopperEdgeClearance=p.FromMM(.3)
 nc=ds.m_NetSettings.GetDefaultNetclass();nc.SetClearance(p.FromMM(.15));nc.SetTrackWidth(p.FromMM(.2));nc.SetViaDiameter(p.FromMM(.6));nc.SetViaDrill(p.FromMM(.3))
 # USB tab follows the connector at x48; all manufactured geometry is explicit.
 outline=[(0,0),(42,0),(42,-2.5),(54,-2.5),(54,0),(96,0),(96,112),(0,112),(0,0)]
 for a,c in zip(outline,outline[1:]):edge(b,a,c)
 tree=ET.parse(R/'exports/OpenKeys-netlist.xml').getroot();nets={};pin_net={}
 for n in tree.findall('./nets/net'):
  name=n.attrib['name']
  if name.startswith('unconnected-'):continue
  z=p.NETINFO_ITEM(b,name);b.Add(z);nets[name]=z
  for node in n.findall('node'):pin_net[(node.attrib['ref'],node.attrib['pin'])]=name
 comps=list(csv.DictReader((R/'BOM.csv').open()));fps={}
 groups={'01-usb':(49,8,'B'),'02-power':(13,20,'B'),'03-mcu':(50,24,'B'),'04-se-storage':(84,58,'F'),'05-nfc':(27,51,'F'),'06-trusted-ui':(60,18,'B'),'07-fault-tamper':(69,24,'B'),'08-input-guard':(21,8,'B'),'09-display-power':(82,28,'B'),'10-sys-window':(25,38,'B'),'11-power-measurement':(40.5,23.5,'F'),'12-active-reset':(72,32,'F')}
 def place(f,x,y,angle,side):
  if side=='B' and f.GetLayer()==p.F_Cu:f.Flip(f.GetPosition(),p.FLIP_DIRECTION_LEFT_RIGHT)
  f.SetOrientationDegrees(angle);f.SetPosition(v(x,y))
 for c in comps:
  ref=c['ref'];lib,name=c['footprint'].split(':',1);path=str(CAD/'OpenKeys.pretty') if lib=='OpenKeys' else '/usr/share/kicad/footprints/'+lib+'.pretty'
  f=p.FootprintLoad(path,name)
  if not f:raise ValueError((ref,c['footprint']))
  f.SetReference(ref);f.SetValue(c['value']);f.SetDNP(c['dnp']=='True');b.Add(f);fps[ref]=f
  for pad in f.Pads():
   net=pin_net.get((ref,pad.GetNumber()))
   if net:pad.SetNet(nets[net])
  f.Reference().SetVisible(False);f.Value().SetVisible(False)
  if ref in PLACEMENT:place(f,*PLACEMENT[ref])
  else:
   x,y,side=groups[c['sheet']]
   key=next((n for (rr,_),n in pin_net.items() if rr==ref and n.startswith('BTN')),None)
   if key:
    digit=int(key[3:-2]);xx,yy=KEYS[str(digit+1 if digit else 11)];x,y,side=xx+4,yy+3,'F'
   place(f,x,y,0,side)
 # Exact MCU decoupling pins, with isolated VCAP capacitors.
 dec={'C9':(19,0),'C10':(32,0),'C11':(48,0),'C12':(64,0),'C13':(64,2.3),'C15':(30,0),'C16':(62,0),'C17':(13,0)}
 for ref,(number,tangent) in dec.items():
  pad=next(z for z in fps['U4'].Pads() if z.GetNumber()==str(number));xx,yy=mm(pad.GetPosition());dx,dy=xx-50,yy-24
  if abs(dx)>abs(dy):xx+=math.copysign(3.5,dx);yy+=tangent;angle=0
  else:yy+=math.copysign(3.5,dy);xx+=tangent;angle=90
  place(fps[ref],xx,yy,angle,'B')
 pack_passives(b,fps)
 for ref,(number,_) in dec.items():
  f=fps[ref];target=next(z.GetPosition() for z in fps['U4'].Pads() if z.GetNumber()==str(number))
  positive=next(z for z in f.Pads() if z.GetNumber()=='1')
  before=(positive.GetPosition()-target).EuclideanNorm();angle=f.GetOrientationDegrees()
  f.SetOrientationDegrees(angle+180)
  if (positive.GetPosition()-target).EuclideanNorm()>=before:f.SetOrientationDegrees(angle)
 for i,(x,y) in enumerate([(3,3),(93,3),(27,108),(93,108)],1):
  f=p.FootprintLoad('/usr/share/kicad/footprints/MountingHole.pretty','MountingHole_2.2mm_M2');f.SetReference('H'+str(i));f.SetPosition(v(x,y));f.Reference().SetVisible(False);f.Value().SetVisible(False);b.Add(f)
 for z in fps['J1'].GraphicalItems():
  if z.GetLayer()==p.F_SilkS:
   a,c=mm(z.GetStart()),mm(z.GetEnd());z.SetStart(v(a[0],max(-2.15,a[1])));z.SetEnd(v(c[0],max(-2.15,c[1])))
 for item in list(fps['J3'].GraphicalItems()):
  if item.GetLayer() in (p.F_SilkS,p.B_SilkS):fps['J3'].RemoveNative(item)
 # Continuous core reference, never under the rotated NFC winding.
 polygon=[(.3,.3),(42.3,.3),(42.3,-2.2),(53.7,-2.2),(53.7,.3),(95.7,.3),(95.7,111.7),(19.3,111.7),(19.3,42.7),(.3,42.7)]
 for layer in (p.In1_Cu,p.In2_Cu):
  z=p.ZONE(b);z.SetLayer(layer);z.SetNet(nets['GND']);z.SetLocalClearance(p.FromMM(.2));z.SetMinThickness(p.FromMM(.15));z.SetPadConnection(p.ZONE_CONNECTION_FULL);z.Outline().NewOutline()
  for xx,yy in polygon:q=v(xx,yy);z.Outline().Append(q.x,q.y)
  b.Add(z)
 rf=fps['J4'];ends=[z.GetPosition() for z in rf.Pads() if z.GetNumber()=='2']
 z=p.PCB_TRACK(b);z.SetStart(ends[0]);z.SetEnd(ends[1]);z.SetWidth(p.FromMM(.5));z.SetLayer(p.B_Cu);z.SetNet(nets['ANT2']);b.Add(z)
 def trace(net,points,width,layer=p.B_Cu):
  for a,c in zip(points,points[1:]):
   if math.dist(a,c)<.0001:continue
   z=p.PCB_TRACK(b);z.SetStart(v(*a));z.SetEnd(v(*c));z.SetLayer(layer);z.SetWidth(p.FromMM(width));z.SetNet(nets[net]);b.Add(z)
 def pin(ref,n):return mm(next(z.GetPosition() for z in fps[ref].Pads() if z.GetNumber()==str(n)))
 def via(net,point):
  z=p.PCB_VIA(b);z.SetPosition(v(*point));z.SetWidth(p.F_Cu,p.FromMM(.6));z.SetDrill(p.FromMM(.3));z.SetViaType(p.VIATYPE_THROUGH);z.SetLayerPair(p.F_Cu,p.B_Cu);z.SetNet(nets[net]);b.Add(z)
 # Reserve the converter and isolated VCAP connections BEFORE fanout/routing.
 for name,n,ind,yy in [('SW_L1',4,1,23.85),('SW_L2',2,2,26.15)]:
  a=pin('U3',n);c=pin('L1',ind)
  assert next(z.GetNetname() for z in fps['U3'].Pads() if z.GetNumber()==str(n))==name
  assert math.dist(c,(15.5,yy))<.001
  trace(name,[a,(17,a[1]),(16.35,yy),c],.2)
 for ref,n in [('C15',30),('C16',62)]:
  a,c=pin('U4',n),pin(ref,1)
  mid=(c[0],a[1]) if abs(a[0]-50)>abs(a[1]-24) else (a[0],c[1])
  trace('VCAP1' if n==30 else 'VCAP2',[a,mid,c],.25)
 # Reserve fine-pitch signal escapes before the ground fanout can surround
 # a pad. The router sees and must preserve clearance to these real tracks.
 for net,ref,n,end,layer in [
  ('REG_VINA','U3',8,(23,25),p.B_Cu),
  ('USB_GUARD_ILM','U14',9,(23.3,8.225),p.B_Cu),
  ('SE_CORE','U5',18,(84,52),p.F_Cu),
  ('SE_CORE','U5',15,(87,54.2),p.F_Cu),
  ('NFC_MOSI','U7',8,(25,54.2),p.F_Cu),
  ('VBUS_RAW','J1','A4',(50.45,6),p.F_Cu),
  ('VBUS_RAW','J1','A9',(45.55,6),p.F_Cu),
  ('BTN5_N','U4',37,(57.3,22.25),p.B_Cu),
  ('BTN4_N','U4',57,(49.75,30.75),p.B_Cu),
 ]:
  a=pin(ref,n);trace(net,[a,end],.15,layer);via(net,end)
 # The host-side escapes preserve the reviewed inner-layer bus topology.
 # Staggered straight escapes cross that bus on this dense layout.
 buffer_escapes={1:[(86.416,20.975),(86.416,20.5)],
  2:[(87.558,20.325),(87.558,19.7752)],
  3:[(91.186,19.675),(91.186,18.6088)],4:[(87.05,19.025)],
  5:[(94.6,19.025)],6:[(95.1,19.675)],7:[(94.6,20.325)],8:[(95.1,20.975)]}
 for n,points in buffer_escapes.items():
  net=pin_net[('U21',str(n))];trace(net,[pin('U21',n),*points],.15);via(net,points[-1])
 for net,n,path in [
  ('ANT1',3,[(25,49.6),(25,47.5)]),
  ('NFC_VOUT',2,[(25.5,49.6),(25.5,48.8),(26.2,48.1)]),
  ('ANT2',4,[(24.5,49.6),(24.5,48.8),(23.8,48.1)]),
 ]:
  assert math.dist(pin('U7',n),path[0])<.001
  trace(net,path,.15,p.F_Cu);via(net,path[-1])
 project=json.loads((CAD/'OpenKeys.kicad_pro').read_text());project['board']['design_settings']['rules']={'min_clearance':.15,'min_track_width':.15,'min_via_diameter':.6,'min_via_annular_width':.15,'min_through_hole_diameter':.3,'min_hole_clearance':.25,'min_copper_edge_clearance':.3,'solder_mask_to_copper_clearance':0.0}
 project['board']['design_settings']['drc_exclusions']=[]
 project['net_settings']={'meta':{'version':4},'classes':[{'name':'Default','clearance':.15,'track_width':.2,'via_diameter':.6,'via_drill':.3,'diff_pair_width':.15,'diff_pair_gap':.15,'diff_pair_via_gap':.25,'bus_width':12,'wire_width':6}],'netclass_patterns':[]}
 (CAD/'OpenKeys.kicad_pro').write_text(json.dumps(project,indent=2)+'\n');p.SaveBoard(str(CAD/'OpenKeys.kicad_pcb'),b)
 write_placement(b);export_dsn();print(f'R05 placed {len(fps)} components; saved board is not yet routed')

def write_placement(b):
 with (R/'review/placement.csv').open('w') as out:
  w=csv.writer(out,lineterminator='\n');w.writerow(['ref','x_mm','y_mm','rotation_deg','side','dnp'])
  for f in sorted(b.GetFootprints(),key=lambda f:f.GetReference()):w.writerow([f.GetReference(),*mm(f.GetPosition()),round(f.GetOrientationDegrees(),3),'F' if f.GetLayer()==p.F_Cu else 'B',f.IsDNP()])

def export_dsn():
 b=p.LoadBoard(str(CAD/'OpenKeys.kicad_pcb'))
 for z in list(b.GetDrawings()):
  if z.GetLayer()==p.Edge_Cuts:b.RemoveNative(z)
 polygon=[(0,0),(42,0),(42,-2.5),(54,-2.5),(54,0),(96,0),(96,112),(19,112),(19,43),(0,43),(0,0)]
 for a,c in zip(polygon,polygon[1:]):edge(b,a,c)
 for z in list(b.GetTracks()):
  # Keep the existing in-core chip feed. Only the winding underpass crosses
  # the excluded RF region and is restored separately on SES import.
  if z.GetNetname()=='ANT2' and min(mm(z.GetStart())[0],mm(z.GetEnd())[0])<19:b.RemoveNative(z)
 for z in list(b.Zones()):
  if z.GetLayer()==p.In2_Cu:b.RemoveNative(z)
 rf=next(f for f in b.GetFootprints() if f.GetReference()=='J4')
 for z in list(rf.GraphicalItems()):
  if z.GetLayer() in (p.F_Cu,p.B_Cu):rf.RemoveNative(z)
 for z in list(rf.Pads()):
  if z.GetNumber()=='1':
   # A virtual circular routing terminal wholly INSIDE the actual 3x0.5mm
   # copper feed. The final board keeps the original rectangular pad; SES
   # imports tracks only. This exposes its in-core copper to the router.
   z.SetPosition(v(19.5,46));z.SetShape(p.PAD_SHAPE_CIRCLE);z.SetSize(p.VECTOR2I(p.FromMM(.5),p.FromMM(.5)))
  elif mm(z.GetPosition())[0]<19:rf.RemoveNative(z)
 if not p.ExportSpecctraDSN(b,str(R/'review/OpenKeys-core.dsn')):raise RuntimeError('DSN export failed')
 path=R/'review/OpenKeys-core.dsn';text=path.read_text().replace('(layer In1.Cu\n      (type signal)','(layer In1.Cu\n      (type power)')
 # Freerouting Wiring.calcFixed accepts "normal" as UNFIXED; KiCad's
 # "route" token falls through to USER_FIXED. Preserve critical copper and
 # ground fanout, but let ordinary routes move around a new component ECO.
 # The result is always rechecked in KiCad; this is not a DRC exemption.
 protected={'ANT1','ANT2','SW_L1','SW_L2','VCAP1','VCAP2','USB_DP','USB_DM','VDDA','LSE_IN','LSE_OUT'}
 lines=[]
 for line in text.splitlines():
  if '(type route)' in line:
   fixed=any('(net '+net+')' in line for net in protected) or ('(via ' in line and '(net GND)' in line)
   line=line.replace('(type route)','(type fix)' if fixed else '(type normal)')
  lines.append(line)
 text='\n'.join(lines)+'\n'
 settings='    (autoroute_settings (fanout off) (autoroute on) (postroute off) (vias on) (via_costs 50) (plane_via_costs 5) (start_ripup_costs 100) (start_pass_no 1)\n'
 for layer,direction in [('F.Cu','vertical'),('In1.Cu','vertical'),('In2.Cu','horizontal'),('B.Cu','vertical')]:settings+='      (layer_rule '+layer+' (active '+('off' if layer=='In1.Cu' else 'on')+') (preferred_direction '+direction+') (preferred_direction_trace_costs 1) (against_preferred_direction_trace_costs 2.2))\n'
 settings+='    )\n';path.write_text(text.replace('    (boundary\n',settings+'    (boundary\n',1))

def import_routes(path):
 b=p.LoadBoard(str(CAD/'OpenKeys.kicad_pcb'))
 if not p.ImportSpecctraSES(b,str(path)):raise RuntimeError('Session import failed')
 # The RF-only underpass is outside the routing core and is not in SES.
 for t in list(b.GetTracks()):
  if t.GetNetname()=='ANT2' and t.GetLayer()==p.B_Cu:
   # Retain only routing-core segments; the fixed winding link is restored.
   if min(mm(t.GetStart())[0],mm(t.GetEnd())[0])<19:b.RemoveNative(t)
 rf=next(f for f in b.GetFootprints() if f.GetReference()=='J4')
 ends=[z.GetPosition() for z in rf.Pads() if z.GetNumber()=='2']
 z=p.PCB_TRACK(b);z.SetStart(ends[0]);z.SetEnd(ends[1]);z.SetWidth(p.FromMM(.5));z.SetLayer(p.B_Cu);z.SetNet(b.FindNet('ANT2'));b.Add(z)
 if not p.ZONE_FILLER(b).Fill(b.Zones()):raise RuntimeError('Fill failed')
 p.SaveBoard(str(CAD/'OpenKeys.kicad_pcb'),b);write_placement(b);print('R05 route import complete')

if __name__=='__main__':
 a=argparse.ArgumentParser();a.add_argument('--import-ses',type=Path);a.add_argument('--export-dsn',action='store_true');args=a.parse_args()
 if args.import_ses:import_routes(args.import_ses)
 elif args.export_dsn:export_dsn()
 else:build()
