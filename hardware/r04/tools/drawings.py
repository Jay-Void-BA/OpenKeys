#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""Dimensioned assembly drawing; PCB linework comes from the actual board."""
from pathlib import Path
from html import escape
import json
import pcbnew as p
R=Path(__file__).resolve().parents[1]
b=p.LoadBoard(str(R/'kicad/OpenKeys.kicad_pcb'));fps={f.GetReference():f for f in b.GetFootprints()}
def loc(a):return (p.ToMM(a.x)-100,p.ToMM(a.y)-80)
def case(ref):return [round(v+3,4) for v in loc(fps[ref].GetPosition())]
buttons=[case('SW'+str(i)) for i in range(2,6)];holes=[case('H'+str(i)) for i in range(1,5)]
assert buttons==[[13,41.5],[26,41.5],[13,62],[26,62]]
assert holes==[[6,6],[68,6],[6,70],[68,70]] and case('SW6')==[9,56]
data={'revision':'0.4','units':'mm','case':[74,98,20],'wall':1.8,'floor':1.6,'lid':1.6,'pcb_offset':[3,3,11],'pcb_core':[68,92,1],'usb_tongue_y':-2.5,'buttons':buttons,'mount_holes':holes,'hole_diameter':2.2,
 'fingerprint':{'center':[52.3,51.3],'flange_diameter':14.0,'max_head_neck_width':14.3,'face_diameter':12.1,'opening_diameter':12.4,'seat_diameter':14.6,'max_thickness':1.5,'flex_total_length':41,'center_to_flex_tip':34,'flex_width':6.5,'connector':case('J5'),'orientation':'face up;tail left;contacts down;pin1 lower edge'},
 'lcd':{'pin1':case('U8'),'overall_origin':[11.35,10],'overall_width':56.8,'depth':[5.0,6.0],'window':[13.65,11.5,46.7,15.3],'cover_window':[12.5,10.35,49,17.6,.8]},
 'cell':{'part':'Adafruit1578 protected500mAh','body':[29,36,4.75],'position':[19.5,37,1.8],'clearance_envelope':[18,36,32.8,38.8,6.5],'thermistor':'Semitec103AT-2;insulated contact to pouch'},'cad_3d_transform':{'x':'drawing_x','y':'98-drawing_y','z':'height_from_case_back'},'rf_case_y_min':76,'assembly_qualified':False,'security_qualified':False}
(R/'mechanical/dimensions.json').write_text(json.dumps(data,indent=2)+'\n')
out=['<svg xmlns="http://www.w3.org/2000/svg" width="1700" height="1200" viewBox="0 0 340 240"><rect width="340" height="240" fill="#f5f8fb"/><g font-family="DejaVu Sans,Arial">']
def rect(x,y,w,h,fill='none',stroke='#29475b',r=0):out.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill}" stroke="{stroke}" stroke-width=".3"/>')
def txt(x,y,s,size=2.7,anchor='start',color='#29475b'):out.append(f'<text x="{x}" y="{y}" font-size="{size}" text-anchor="{anchor}" fill="{color}">{escape(s)}</text>')
def line(a,c,color='#6c8799',width=.3):out.append(f'<path d="M{a[0]},{a[1]}L{c[0]},{c[1]}" fill="none" stroke="{color}" stroke-width="{width}"/>')
def circle(x,y,r,fill,stroke='#29475b'):out.append(f'<circle cx="{x}" cy="{y}" r="{r}" fill="{fill}" stroke="{stroke}" stroke-width=".25"/>')
txt(12,14,'OPENKEYS R0.4',7);txt(12,22,'ASSEMBLY / ELECTRICAL PCB CANDIDATE - PHYSICAL QUALIFICATION REQUIRED',3)
txt(328,14,'74 x 98 x 20 mm',5,'end');txt(328,22,'USB-C female / white16x2 LCD / device fingerprint',2.7,'end')
x,y=24,42;rect(x,y,74,98,'#dbe5ec',r=5);rect(x+32.1,y-.3,9.8,1.2,'#627c8e',r=.4)
rect(x+12.5,y+10.35,49,17.6,'#e9edf1');rect(x+13.65,y+11.5,46.7,15.3,'#f4fbf6')
txt(x+37,y+18.2,'OpenKeys',3.3,'middle');txt(x+37,y+23.4,'Use fingerprint',2.8,'middle')
for i,(xx,yy) in enumerate(buttons,1):circle(x+xx,y+yy,2.8,'#25838c');txt(x+xx,y+yy+1,str(i),3,'middle','#fff')
circle(x+52.3,y+51.3,7.3,'none');circle(x+52.3,y+51.3,6.05,'#263f50');txt(x+52.3,y+52.3,'FP',3,'middle','#fff')
for xx,yy in holes:circle(x+xx,y+yy,1.6,'#8ca3b4')
rect(x+3,y+76,68,19,'none','#bb8825',2);txt(x+37,y+86,'NFC',3.4,'middle','#926615')
line((x,y-6),(x+74,y-6));txt(x+37,y-8,'74.0',3,'middle');line((x-6,y),(x-6,y+98));txt(x-8,y+49,'98.0',3,'end');txt(x+37,y+105,'FRONT',3.4,'middle')
for yy,title,detail in [(15,'White-backlit character LCD','16x2 English; bonded0.8mm polycarbonate lens'),(39,'Contextual four-key PIN','Fingerprint first; PIN additional, never fallback'),(51,'FPC2534AM','12.4mm aperture /14.6mm flange seat'),(66,'Backup rail + lid contact','Validate preload, retention and independent LCD reset'),(84,'RF region','No battery, shield, screw or plane behind the coil')]:
 line((x+74,y+yy),(109,y+yy));txt(113,y+yy-1,title,3);txt(113,y+yy+4,detail,2.25)
xx,yy=246,43;rect(xx,yy,68,92,'#e5f0e9','#367457')
colors={p.F_Cu:'#b98124',p.B_Cu:'#78a5b6',p.In2_Cu:'#bfa8cf'}
for t in b.GetTracks():
 if t.GetClass()!='PCB_VIA' and t.GetLayer() in colors:
  a=loc(t.GetStart());c=loc(t.GetEnd());line((xx+a[0],yy+a[1]),(xx+c[0],yy+c[1]),colors[t.GetLayer()],max(.08,p.ToMM(t.GetWidth())*.6))
for z in fps['J4'].GraphicalItems():
 if z.GetLayer()==p.F_Cu:
  a=loc(z.GetStart());c=loc(z.GetEnd());line((xx+a[0],yy+a[1]),(xx+c[0],yy+c[1]),'#b98124',.5)
for ref in ['U4','U5','U6','U7','U15','J5']:
 px,py=loc(fps[ref].GetPosition());txt(xx+px,yy+py,ref,1.9,'middle')
rect(xx+8.35,yy+7,56.8,26.35,'none','#667d8c');txt(xx+34,yy+17,'LCD ABOVE',2.5,'middle')
txt(xx+34,yy+100,'ACTUAL PCB LINEWORK',2.7,'middle');txt(xx+34,yy+106,'F.Cu / B.Cu / In2 signals; In1 ground',2.15,'middle')
x,y=24,179;rect(x,y,98,20,'#dbe5ec',r=2);rect(x+1.8,y+1.6,94.4,16.8,'#f5f8fb',r=1)
rect(x+3,y+8,92,1,'#46845c');rect(x+37,y+13.45,36,4.75,'#b7c4cf');rect(x+10,y+2,18.9,5.5,'#e2e6d9');rect(x+44.3,y-.1,14,1.5,'#263f50')
line((x+76,y-2),(x+76,y+22),'#bb8825');txt(x+86,y+8,'RF',2.8,'middle');txt(x+101,y+11,'20.0',3);txt(x+49,y+28,'SECTION / PCB1.0 / wall1.8 / floor1.6',2.7,'middle')
txt(155,164,'ASSEMBLY NOTES',4)
for i,s in enumerate(['PCB and enclosure controls checked together.', 'FP tail left, contacts down; pin1 at lower edge.', 'Support sensor back; do not clamp the flex bend.', '500mAh protected pack plus attached10k NTC.', '0.50W input budget; cell charge up to0.42W.', 'No IP rating or physical-attack certificate.']):txt(155,173+i*6,s,2.5)
rect(12,224,316,9,'#fff0d3','#bb8825',1);txt(170,230,'CAD AND SIMULATION ARE ENGINEERING EVIDENCE / QUALIFY BEFORE REAL KEY PROVISIONING',2.75,'middle','#805b16')
out.append('</g></svg>');(R/'exports/OpenKeys-dimensions.svg').write_text('\n'.join(out)+'\n')
print('Generated PCB-derived dimensions and assembly drawing; physical fit remains unqualified')
