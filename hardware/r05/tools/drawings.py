#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""Dimensioned R05 assembly candidate; key/coil linework comes from saved PCB."""
from html import escape
import json
from pathlib import Path
import pcbnew as p

R = Path(__file__).resolve().parents[1]
b = p.LoadBoard(str(R / 'kicad/OpenKeys.kicad_pcb'))
fps = {f.GetReference(): f for f in b.GetFootprints()}
def loc(a): return (round(p.ToMM(a.x)-100, 4), round(p.ToMM(a.y)-80, 4))
def case(ref): return [round(x+3, 4) for x in loc(fps[ref].GetPosition())]
keys = [case('SW'+str(i)) for i in range(2,12)]
holes = [case('H'+str(i)) for i in range(1,5)]
assert keys == [[39,53],[57,53],[75,53],[39,67],[57,67],[75,67],
                [39,81],[57,81],[75,81],[57,95]]
assert case('SW12') == [89,105.5] and case('SW1') == [98,68]
active_contacts=[case(ref) for ref in ['SW13','SW14','SW15']]
assert active_contacts==[[84.25,52],[36,103],[80,108]]
data = {
    'revision':'0.5','units':'mm','case':[102,118,30], 'wall':1.8,
    'floor':1.6,'lid':1.6,'pcb_offset':[3,3,11], 'pcb_body':[96,112,1],
    'pcb_envelope':[96,114.5,1], 'usb_tab_y_min':-2.5,
    'keys':keys, 'key_labels':[1,2,3,4,5,6,7,8,9,0],
    'mount_holes':holes,'hole_diameter':2.2,'lid_contact':case('SW12'),
    'active_lid_contacts':active_contacts,
    'backup_cell':{'part':'Panasonic CR-2032/HFN, pre-tabbed primary',
                   'body_centre':[85,103], 'nominal_body':[20,3.2],
                   'nominal_z':7.2,'polarity_pad':case('BT1'),
                   'charging_allowed':False,
                   'retention':'Insulated internal retention; inspect exact tab/body height and sample; do not compress cell'},
    'mechanical_power_switch':case('SW1'),
    'lcd':{'part':'DFRobot DFR0555 V1.1','module_origin':[7.5,11,15],
           'module_envelope':[87,32,13], 'window':[8,11.5,86,31],
           'cover':[6.5,10,89,34,.8], 'connector':case('J7'),
           'hole_pattern':'Not assumed; inspect sample before retaining/printing final lid',
           'supply':'Switched SYS; static window3.3746..4.8596V under explicit assumptions; physical NOT qualified'},
    'cell':{'part':'Protected 1S 500mAh, pack + 10k NTC',
            'body':[29,36,4.75],'origin':[39,53,1.8],
            'pcb_back_keepout':[33,47,35,43]},
    'rf_keepout_pcb':[0,43,19,69],
    'assembly_qualified':False, 'security_qualified':False,
}
(R/'mechanical').mkdir(exist_ok=True)
(R/'mechanical/dimensions.json').write_text(json.dumps(data,indent=2)+'\n')
out=['<svg xmlns="http://www.w3.org/2000/svg" width="1900" height="1300" viewBox="0 0 380 260">',
     '<rect width="380" height="260" fill="#f3f6fa"/><g font-family="DejaVu Sans,Arial">']
def rect(x,y,w,h,fill='none',stroke='#284558',r=0):
    out.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill}" stroke="{stroke}" stroke-width=".3"/>')
def text(x,y,value,size=2.7,anchor='start',color='#284558'):
    out.append(f'<text x="{x}" y="{y}" font-size="{size}" text-anchor="{anchor}" fill="{color}">{escape(value)}</text>')
def line(a,c,color='#91a6b6',width=.3):
    out.append(f'<path d="M{a[0]},{a[1]}L{c[0]},{c[1]}" fill="none" stroke="{color}" stroke-width="{width}"/>')
def circle(x,y,r,fill):
    out.append(f'<circle cx="{x}" cy="{y}" r="{r}" fill="{fill}" stroke="#284558" stroke-width=".25"/>')
text(12,13,'OPENKEYS R0.5',7)
text(12,21,'PIN + TEN KEYS / BLUE 16x2 LCD / NO FINGERPRINT',3)
text(368,13,'102 x 118 x 30 mm',5,'end')
text(368,21,'DIMENSIONED CAD CANDIDATE - NOT A PHYSICAL SAMPLE',2.8,'end')
x,y=22,42
rect(x,y,102,118,'#dce5ed',r=5)
rect(x+46,y-.4,10,1.4,'#657f91',r=.5)
rect(x+6.5,y+10,89,34,'#bdcbd8')
rect(x+8,y+11.5,86,31,'#154bb1')
text(x+51,y+24,'OpenKeys R05',4,'middle','#fff')
text(x+51,y+33,'Enter device PIN',3.4,'middle','#fff')
for digit,(xx,yy) in zip(data['key_labels'],keys):
    circle(x+xx,y+yy,3.1,'#267b8b');text(x+xx,y+yy+1.3,str(digit),3.7,'middle','#fff')
for xx,yy in holes:circle(x+xx,y+yy,1.3,'#9eb0bd')
rect(x+5,y+49,14,61,'none','#b18028',2)
text(x+12,y+78,'NFC',2.6,'middle','#8b621e')
rect(x+100.5,y+64,1.8,8,'#657f91')
line((x,y-7),(x+102,y-7));text(x+51,y-9,'102.0',3,'middle')
line((x-7,y),(x-7,y+118));text(x-9,y+59,'118.0',3,'end')
text(x+51,y+126,'FRONT / ALL LABELS IN ENGLISH',3,'middle')
for yy,title,detail in [
    (15,'DFRobot DFR0555','87 x 32 x 13; blue LCD / white text'),
    (36,'Latched display power','Reset cuts supply + both I2C wires'),
    (54,'Ten independent contacts','0-9 direct GPIO; no keyboard matrix'),
    (77,'Contextual controls','PIN: hold 9 erase; hold 0 cancel'),
    (94,'Request review','2 previous / 8 next / 5 confirm / 0 cancel'),
    (111,'Four lid contacts + backup cell','Active loop and separate hard reset; test preload')]:
    line((x+102,y+yy),(135,y+yy));text(139,y+yy-1,title,3)
    text(139,y+yy+4,detail,2.3)
xx,yy=268,43
rect(xx,yy,96,112,'#e4eee8','#367457')
colors={p.F_Cu:'#b78325',p.B_Cu:'#7cacc0',p.In2_Cu:'#b3a0ca'}
for t in b.GetTracks():
    if t.GetClass()=='PCB_TRACK' and t.GetLayer() in colors:
        a,c=loc(t.GetStart()),loc(t.GetEnd())
        line((xx+a[0],yy+a[1]),(xx+c[0],yy+c[1]),colors[t.GetLayer()],max(.07,p.ToMM(t.GetWidth())*.6))
for z in fps['J4'].GraphicalItems():
    if z.GetLayer()==p.F_Cu:
        a,c=loc(z.GetStart()),loc(z.GetEnd())
        line((xx+a[0],yy+a[1]),(xx+c[0],yy+c[1]),'#b78325',.5)
for ref in ['U4','U5','U6','U7','U16','U17','U18','U21','U22','U24','U27','J7']:
    px,py=loc(fps[ref].GetPosition());text(xx+px,yy+py,ref,1.9,'middle')
rect(xx+4.5,yy+8,87,32,'none','#677e8d')
circle(xx+82,yy+100,10,'#d3d9df')
text(xx+82,yy+96,'BACKUP CELL',1.9,'middle')
text(xx+82,yy+98,'BACK SIDE',1.5,'middle')
for ref in ['SW12','SW13','SW14','SW15']:
    px,py=loc(fps[ref].GetPosition());circle(xx+px,yy+py,1.5,'#b95042');text(xx+px,yy+py-2,ref,1.7,'middle')
text(xx+48,yy+24,'LCD ABOVE',2.7,'middle')
text(xx+48,yy+120,'ACTUAL PCB COPPER LINEWORK',2.7,'middle')
text(xx+48,yy+126,'4 layers / 1.0 mm / integral NFC loop',2.3,'middle')
x,y=22,198
rect(x,y,118,30,'#dce5ed',r=2)
rect(x+1.8,y+1.6,114.4,26.8,'#f3f6fa')
rect(x+3,y+18,112,1,'#548768')
rect(x+11,y+2,32,13,'#bec9d6')
rect(x+53,y+23.45,36,4.75,'#a9b8c6')
rect(x+93,y+19.6,20,3.2,'#d3d9df')
text(x+59,y+38,'SECTION / 30.0 DEPTH / PCB Z=11',3,'middle')
text(160,190,'ASSEMBLY AND SECURITY LIMITS',3.8)
for i,s in enumerate([
    'One USB-C female data / 5 V charge socket. No PD request.',
    '100 mA input budget: 0.50 W; cell charge target up to 0.42 W.',
    'Protected 500 mAh pack + attached 10k NTC; not a bare cell.',
    'No metal or battery behind the left-side NFC winding.',
    'LCD aperture, retention, harness and key travel need a sample.',
    'Schematic/DRC/models do not prove fault or side-channel resistance.']):
    text(160,199+i*6,s,2.5)
rect(12,245,356,9,'#fff0d3','#b18028',1)
text(190,251,'ENGINEERING SAMPLE ONLY / NO OWNER SECRETS UNTIL ROOT, PROVISIONING AND PHYSICAL TESTS PASS',2.7,'middle','#805b16')
out.append('</g></svg>')
(R/'exports/OpenKeys-dimensions.svg').write_text('\n'.join(out)+'\n')
print('PCB-derived R05 drawing and dimensions written; physical fit NOT qualified')
