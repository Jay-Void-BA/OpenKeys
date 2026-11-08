#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""Dimensioned SVG drawing + an explicitly unrouted KiCad mechanical floorplan."""
from pathlib import Path
import json, math
R=Path(__file__).resolve().parents[1]
parts=[]
def add(s):parts.append(s)
def rect(x,y,w,h,fill='none',stroke='#19384a',r=0,extra=''):
 add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill}" stroke="{stroke}" stroke-width="0.35" {extra}/>')
def text(x,y,s,size=2.8,color='#19384a',anchor='start'):
 add(f'<text x="{x}" y="{y}" font-size="{size}" fill="{color}" text-anchor="{anchor}">{s}</text>')
def line(x,y,x2,y2,color='#5f7a8b',dashed=False):add(f'<path d="M{x},{y} L{x2},{y2}" stroke="{color}" stroke-width="0.3" fill="none" '+('stroke-dasharray="1.4 1" ' if dashed else '')+'/>')
def dimension(x,y,x2,y2,label,vertical=False):
 line(x,y,x2,y2)
 if vertical:
  line(x-1.2,y,x+1.2,y);line(x-1.2,y2,x+1.2,y2);text(x-2,(y+y2)/2,label,2.7,anchor='end')
 else:
  line(x,y-1.2,x,y+1.2);line(x2,y-1.2,x2,y+1.2);text((x+x2)/2,y-2,label,2.7,anchor='middle')
def drawing():
 add('<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="1100" viewBox="0 0 320 220">')
 add('<rect width="320" height="220" fill="#f8fafc"/>')
 add('<g font-family="DejaVu Sans,Arial,sans-serif">')
 text(12,13,'OPENKEYS / R0',7);text(12,21,'MECHANICAL DESIGN REVIEW · 2026-11-08',3.3)
 text(306,14,'44 × 88 × 16 mm',5,anchor='end');text(306,21,'All dimensions in mm · connector/module envelopes provisional',2.8,anchor='end')
 # Front
 x,y=32,48
 rect(x,y,44,88,'#e5edf2',r=5);rect(x+16,y-12,12,17,'#cbd5df',r=.5)
 rect(x+7.5,y+8.5,29,17,'#0c2230',r=.5)
 text(x+10,y+14,'example.com',2.2,'#87f6d4');text(x+10,y+20,'OTP  123 456',3,'#eafff8')
 for xx,yy,label in [(8,38,'↑'),(16,38,'↓'),(8,48,'✓'),(16,48,'×')]:
  add(f'<circle cx="{x+xx}" cy="{y+yy}" r="2.6" fill="#1e8f9c"/>');text(x+xx,y+yy+1,label,2.8,'white','middle')
 rect(x+21.7,y+34.7,16.6,16.6,'#384d5c',r=2)
 for k in range(4):add(f'<path d="M{x+25+k*.6},{y+46} q-2,-{4+k} 3,-{5+k} q{5+k},0 {3+k},{5+k}" fill="none" stroke="#9cbdc7" stroke-width=".25"/>')
 rect(x+2,y+66,40,18,'none','#da941d',2,'stroke-dasharray="1.5 1"');text(x+22,y+74,'NFC RF WINDOW',2.6,'#9b6507','middle');text(x+22,y+79,'no metal behind',2.2,'#9b6507','middle')
 for xx,yy in [(6,6),(38,6),(6,60),(38,60)]:add(f'<circle cx="{x+xx}" cy="{y+yy}" r="1.2" fill="#adc0cc"/>')
 dimension(x,y-18,x+44,y-18,'44.0');dimension(x-9,y,x-9,y+88,'88.0',True)
 text(x+22,y+95,'FRONT',3.6,anchor='middle')
 # Front labels
 for a,b,c in [(14,'Trusted OLED','29 × 17 window, confirm/cancel'),(43,'Fingerprint','16 × 16 envelope; secure module TBD'),(59,'Buttons / device PIN','UP, DOWN, CONFIRM, CANCEL'),(76,'NFC keepout','No battery, shield or ground plane')]:
  line(x+44,y+a,83,y+a);text(86,y+a-1,b,3);text(86,y+a+3,c,2.4,'#667e8d')
 # board floorplan
 x,y=204,48
 rect(x,y,38,82,'#e1f0e9','#37745b',2)
 rect(x+4,y+5,30,24,'none','#466b87',1,'stroke-dasharray="1 1"')
 text(x+19,y+11,'OLED / trusted MCU',2.5,anchor='middle');rect(x+12,y+14,10,10,'#294b40',r=.3);text(x+17,y+20,'U7',2.6,'white','middle')
 rect(x+8,y+28,20,30,'#dbe1e6','#81919b',1,'stroke-dasharray="1 1"');text(x+18,y+54,'Battery below PCB',2.3,anchor='middle')
 rect(x+19,y+32,16,16,'#c0d5cd','#37745b',1);text(x+27,y+39,'FP TBD',2.3,anchor='middle')
 rect(x+2,y+63,34,17,'#fff2d4','#c58a19',1);text(x+19,y+69,'RF ONLY',2.8,'#976006','middle')
 for k in [0,1,2]:rect(x+4+k,y+71+k,30-2*k,7-2*k,'none','#ba781b',2)
 text(x+19,y+84,'COIL ILLUSTRATIVE — NOT TUNED',1.9,'#976006','middle')
 dimension(x,y-10,x+38,y-10,'38.0');dimension(x+45,y,x+45,y+82,'82.0',True)
 text(x+19,y+95,'PCB PARTITION / 4 LAYERS',3.1,anchor='middle')
 # side section
 x,y=25,166
 rect(x,y,88,16,'#e5edf2',r=2);rect(x+2,y+2,84,12,'#f8fafc',r=1)
 rect(x+3,y+8.6,82,1,'#37745b');rect(x+31,y+10.2,30,4,'#bdc9d1');rect(x+7,y+2.8,25,2.4,'#19384a');rect(x+35,y+.2,16,4,'#384d5c')
 line(x+66,y-2,x+66,y+18,'#c58a19',True);text(x+76,y+6,'RF',2.4,'#976006','middle')
 dimension(x+94,y,x+94,y+16,'16.0',True);text(x+44,y+23,'SECTION A–A · enclosure 1.6 mm / PCB 1.0 mm',2.9,anchor='middle')
 text(142,158,'ASSEMBLY NOTES',4)
 for i,t in enumerate(['4 × M2 screw positions: (6,6), (38,6), (6,60), (38,60)', 'Base + lid + four captive plungers; PETG/ABS prototype', 'Protected 1S 200 mAh battery envelope: 20 × 30 × 4', 'NFC screen/fingerprint operation requires battery power', 'Pin order, connector cutouts, tolerance and drop tests pending']):text(142,166+i*5,t,2.5)
 rect(12,204,296,9,'#fff0d7','#e0a347',1)
 text(160,210,'DESIGN REVIEW ONLY · NO GERBER RELEASE · NO CLAIM OF SCA / EMC / FIDO CERTIFICATION',3,'#81550c','middle')
 add('</g></svg>');(R/'exports/OpenKeys-dimensions.svg').write_text('\n'.join(parts)+'\n')

def board():
 import pcbnew as p
 b=p.BOARD();b.GetDesignSettings().SetCopperLayerCount(4);b.GetDesignSettings().SetBoardThickness(p.FromMM(1.0))
 # Board local (0,0) maps to case (3,3), copper restricted in local y>=63.
 ox,oy=100,80
 def v(x,y):return p.VECTOR2I(p.FromMM(x+ox),p.FromMM(y+oy))
 for a,c in [((0,0),(38,0)),((38,0),(38,82)),((38,82),(0,82)),((0,82),(0,0))]:
  seg=p.PCB_SHAPE();seg.SetShape(p.SHAPE_T_SEGMENT);seg.SetStart(v(*a));seg.SetEnd(v(*c));seg.SetLayer(p.Edge_Cuts);seg.SetWidth(p.FromMM(.05));b.Add(seg)
 def label(s,x,y,layer=p.Dwgs_User,size=1):
  t=p.PCB_TEXT(b);t.SetText(s);t.SetPosition(v(x,y));t.SetLayer(layer);t.SetTextSize(p.VECTOR2I(p.FromMM(size),p.FromMM(size)));t.SetTextThickness(p.FromMM(.15));b.Add(t)
 def box(x,y,w,h,layer=p.Dwgs_User):
  z=p.PCB_SHAPE();z.SetShape(p.SHAPE_T_RECT);z.SetStart(v(x,y));z.SetEnd(v(x+w,y+h));z.SetLayer(layer);z.SetWidth(p.FromMM(.15));b.Add(z)
 for x,y in [(3,3),(35,3),(3,57),(35,57)]:
  fp=p.FootprintLoad('/usr/share/kicad/footprints/MountingHole.pretty','MountingHole_2.2mm_M2')
  fp.SetReference('H'+str(len(b.GetFootprints())+1));fp.SetPosition(v(x,y));b.Add(fp)
 box(1,3,36,26);label('OLED + MCU / SE',19,5)
 # Only a verified package footprint is placed, no fabricated black-box pads.
 fp=p.FootprintLoad('/usr/share/kicad/footprints/Package_QFP.pretty','LQFP-64_10x10mm_P0.5mm')
 fp.SetReference('U7');fp.SetValue('STM32U585RIT6');fp.SetPosition(v(18,18));b.Add(fp)
 box(9,28,20,30);label('BATTERY BELOW',19,54)
 box(19,32,16,16);label('TRUSTED FP TBD',27,39,size=.8)
 box(0,63,38,19);label('NFC RF WINDOW',19,66);label('NO OTHER COPPER / METAL',19,69,size=.7)
 # Rule areas on all four layers. Coil copper intentionally absent until tuned.
 z=p.ZONE(b);z.SetIsRuleArea(True);z.SetDoNotAllowTracks(True);z.SetDoNotAllowVias(True);z.SetDoNotAllowCopperPour(True);z.SetDoNotAllowPads(True);z.SetDoNotAllowFootprints(True)
 layers=p.LSET()
 for ly in [p.F_Cu,p.In1_Cu,p.In2_Cu,p.B_Cu]:layers.AddLayer(ly)
 z.SetLayerSet(layers);z.Outline().NewOutline()
 for a in [(0,63),(38,63),(38,82),(0,82)]:pt=v(*a);z.Outline().Append(pt.x,pt.y)
 b.Add(z)
 label('UNROUTED FLOORPLAN - NOT FOR FABRICATION',19,-5,size=1)
 label('Connector/module footprints and RF tuning OPEN',19,-2,size=.8)
 p.SaveBoard(str(R/'kicad/OpenKeys-floorplan.kicad_pcb'),b)
if __name__=='__main__':drawing();board();print('Generated dimensioned SVG and unrouted KiCad floorplan')
