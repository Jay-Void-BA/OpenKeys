#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""R0.3 dimensioned enclosure review and intentionally UNROUTED placement study."""
from pathlib import Path
from html import escape
import json
R=Path(__file__).resolve().parents[1]
W,H,D=74,98,20
BUTTONS=[(14,46),(26,46),(14,60),(26,60)]
HOLES=[(6,6),(68,6),(6,70),(68,70)]
parts=[]
def rect(x,y,w,h,fill='none',stroke='#29475b',r=0):parts.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill}" stroke="{stroke}" stroke-width=".35"/>')
def text(x,y,t,size=2.7,anchor='start',color='#29475b'):parts.append(f'<text x="{x}" y="{y}" font-size="{size}" text-anchor="{anchor}" fill="{color}">{escape(t)}</text>')
def line(x,y,xx,yy):parts.append(f'<path d="M{x},{y}L{xx},{yy}" stroke="#6e8695" stroke-width=".3"/>')
def drawing():
 parts.append('<svg xmlns="http://www.w3.org/2000/svg" width="1700" height="1200" viewBox="0 0 340 240"><rect width="340" height="240" fill="#f6f8fb"/><g font-family="DejaVu Sans,Arial">')
 text(12,14,'OPENKEYS R0.3',7);text(12,22,'DIMENSIONED ENGINEERING REVIEW / NOT FOR MANUFACTURE',3)
 text(328,14,'74 x 98 x 20 mm',5,'end');text(328,22,'Single USB-C / 16x2 white LCD / fingerprint mandatory',2.8,'end')
 x,y=25,42
 rect(x,y,W,H,'#dbe5ec',r=5);rect(x+31.9,y-.7,10.2,2.2,'#8495a3',r=.7)
 rect(x+13.65,y+11.5,46.7,15.3,'#f8faf5',r=.8)
 text(x+37,y+17.8,'OpenKeys',3.5,'middle','#202b28');text(x+37,y+23.8,'Use fingerprint',2.9,'middle','#202b28')
 for i,(xx,yy) in enumerate(BUTTONS):
  parts.append(f'<circle cx="{x+xx}" cy="{y+yy}" r="3" fill="#25838c"/>');text(x+xx,y+yy+1,str(i+1),3,'middle','#fff')
 rect(x+44,y+43,16.6,16.6,'#324858',r=2);text(x+52.3,y+53,'FP',3,'middle','#fff')
 rect(x+3,y+76,68,19,'none','#c08a25',2);text(x+37,y+85,'NFC RF WINDOW',3,'middle','#9b6917')
 for xx,yy in HOLES:parts.append(f'<circle cx="{x+xx}" cy="{y+yy}" r="1.4" fill="#8ca3b4"/>')
 line(x,y-7,x+W,y-7);text(x+W/2,y-9,'74.0',3,'middle');line(x-7,y,x-7,y+H);text(x-9,y+H/2,'98.0',3,'end')
 text(x+W/2,y+105,'FRONT',3.4,'middle')
 for yy,title,detail in [(15,'White-backlit character LCD','16 columns x 2 lines; English; full identity paging'),(49,'Fingerprint + contextual keys','PIN only after fingerprint; PIN never a fallback'),(68,'Lid-open contact + backup rail','Internal actuator; powered-off tamper requires VBAT/LSE'),(85,'RF-only region','No battery, shield, screws or copper planes here')]:
  line(x+W,y+yy,110,y+yy);text(114,y+yy-1,title,3.1);text(114,y+yy+4,detail,2.3)
 # Board partition separate from front to avoid an implied finished PCB.
 xx,yy=246,44;rect(xx,yy,68,92,'#d9eadd','#367457',2)
 rect(xx+9.15,yy+7,56.8,26.35,'none','#5a7487');text(xx+34,yy+18,'LCD envelope',2.8,'middle')
 rect(xx+15,yy+33,32,37,'none','#8a939c');text(xx+31,yy+67,'CELL BELOW',2.3,'middle')
 rect(xx,yy+73,68,19,'#fff0d3','#c08a25',1);text(xx+34,yy+83,'RF ONLY',3,'middle','#9b6917')
 text(xx+34,yy+99,'68 x 92 x 1.0 / 4 LAYERS',2.7,'middle');text(xx+34,yy+105,'UNROUTED PLACEMENT STUDY',2.4,'middle')
 # Side section: all metal remains above the RF cut line.
 x,y=25,177;rect(x,y,98,20,'#dbe5ec',r=2);rect(x+2,y+2,94,16,'#f6f8fb',r=1)
 rect(x+3,y+8,92,1,'#367457');rect(x+36,y+12.2,37,6,'#bac5ce');rect(x+10,y+2.5,26.35,5.5,'#d4d8cb');rect(x+43,y+.4,16.6,4.6,'#324858')
 line(x+76,y-2,x+76,y+22);text(x+86,y+8,'RF',2.8,'middle');text(x+101,y+11,'20.0',3)
 text(x+49,y+28,'SECTION / enclosure1.6mm / PCB1.0mm',2.7,'middle')
 text(155,165,'ASSEMBLY LIMITS',4)
 notes=['LCD selected; FP/body/connector fit needs actual samples.', 'Protected500mAh pack target:32x37x6mm incl PCM/NTC.', 'Polymer shell; four M2 fasteners; captive button plungers.', 'Tamper contact travel/preload must be measured, not assumed.', 'Target mass60-100g is a budget, not measured product weight.', 'No IP rating, physical-attack certification or battery-life claim.']
 for i,t in enumerate(notes):text(155,173+6*i,t,2.6)
 rect(12,224,316,9,'#fff0d3','#c08a25',1);text(170,230,'REVIEW DRAWING / NO GERBER RELEASE / ASSEMBLY + RF + SECURITY TESTS PENDING',3,'middle','#805b16')
 parts.append('</g></svg>');(R/'exports/OpenKeys-dimensions.svg').write_text('\n'.join(parts)+'\n')
def board():
 import pcbnew as p
 b=p.BOARD();b.GetDesignSettings().SetCopperLayerCount(4);b.GetDesignSettings().SetBoardThickness(p.FromMM(1))
 def v(x,y):return p.VECTOR2I(p.FromMM(x+100),p.FromMM(y+80))
 for a,c in [((0,0),(68,0)),((68,0),(68,92)),((68,92),(0,92)),((0,92),(0,0))]:
  z=p.PCB_SHAPE();z.SetShape(p.SHAPE_T_SEGMENT);z.SetStart(v(*a));z.SetEnd(v(*c));z.SetLayer(p.Edge_Cuts);z.SetWidth(p.FromMM(.05));b.Add(z)
 def label(t,x,y,size=1):
  z=p.PCB_TEXT(b);z.SetText(t);z.SetPosition(v(x,y));z.SetLayer(p.Dwgs_User);z.SetTextSize(p.VECTOR2I(p.FromMM(size),p.FromMM(size)));z.SetTextThickness(p.FromMM(.12));b.Add(z)
 def place(ref,lib,fp,x,y,value=''):
  path=str(R/'kicad/OpenKeys.pretty') if lib=='OpenKeys' else '/usr/share/kicad/footprints/'+lib+'.pretty'
  z=p.FootprintLoad(path,fp)
  if not z:raise ValueError((lib,fp))
  z.SetReference(ref);z.SetValue(value);z.SetPosition(v(x,y));b.Add(z)
 for i,(x,y) in enumerate(HOLES):place('H'+str(i+1),'MountingHole','MountingHole_2.2mm_M2',x-3,y-3)
 for ref,lib,fp,x,y,val in [('U4','Package_QFP','LQFP-64_10x10mm_P0.5mm',34,55,'STM32H573RIT6'),('U5','OpenKeys','SE050_SOT1969-1_HX2QFN20',48,63,'SE050E'),('U8','OpenKeys','LCD_NHD_C0216CiZ_P1.5mm',39.25,33.35,'White16x2'),('J1','Connector_USB','USB_C_Receptacle_HRO_TYPE-C-31-M-12',34,1,'USB-C')]:place(ref,lib,fp,x,y,val)
 for i,(x,y) in enumerate(BUTTONS):place('SW'+str(i+2),'Button_Switch_SMD','SW_SPST_TL3342',x-3,y-3)
 place('SW6','Button_Switch_SMD','SW_SPST_TL3342',6,61,'LID CONTACT')
 z=p.ZONE(b);z.SetIsRuleArea(True)
 for fn in ['SetDoNotAllowTracks','SetDoNotAllowVias','SetDoNotAllowCopperPour','SetDoNotAllowPads','SetDoNotAllowFootprints']:getattr(z,fn)(True)
 ls=p.LSET()
 for ly in [p.F_Cu,p.In1_Cu,p.In2_Cu,p.B_Cu]:ls.AddLayer(ly)
 z.SetLayerSet(ls);z.Outline().NewOutline()
 for x,y in [(0,73),(68,73),(68,92),(0,92)]:pt=v(x,y);z.Outline().Append(pt.x,pt.y)
 b.Add(z);label('UNROUTED / NO ELECTRICAL NETS / NOT FOR FABRICATION',34,-5,.9);label('NFC RF ONLY - NO METAL BEHIND',34,82,.9)
 p.SaveBoard(str(R/'kicad/OpenKeys-floorplan.kicad_pcb'),b)
if __name__=='__main__':drawing();board();print('Generated R0.3 dimensions and explicitly unrouted placement study')
