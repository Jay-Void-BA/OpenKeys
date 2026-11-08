#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""Check exported XML independently of the generator. Known missing RF parts
remain explicit blockers; this script never grants manufacturing release.
"""
from pathlib import Path
import xml.etree.ElementTree as ET
import json,re
R=Path(__file__).resolve().parents[1]
x=ET.parse(R/'exports/OpenKeys-netlist.xml').getroot()
ns={n.attrib['name']:{(a.attrib['ref'],a.attrib['pin']) for a in n.findall('node')} for n in x.findall('./nets/net')}
components={c.attrib['ref']:c for c in x.findall('./components/comp')}
checks=[]
def require(name,ok):
 if not ok:raise SystemExit('FAILED: '+name)
 checks.append(name)
def net(ref,pin):
 out=[n for n,v in ns.items() if (ref,str(pin)) in v]
 if len(out)!=1:raise SystemExit(f'{ref}.{pin}: expected exactly1net, got{out}')
 return out[0]
def same(*args):return len({net(*a) for a in args})==1
def value(ref):return components[ref].findtext('value')
def has_part(v,a,b):return any(value(r)==v and ns1=={a,b} for r in components for ns1 in [{n for n,p in ns.items() if any(rr==r for rr,pin in p)}])
require('Only USB-C socket, no USB-A or data mux',sum(value(r)=='HRO TYPE-C-31-M-12' for r in components)==1 and not any('USB-A' in value(r) or 'TS3USB30' in value(r) for r in components))
require('USB-C duplicate data pads joined',same(('J1','A6'),('J1','B6'),('U4',45)) and same(('J1','A7'),('J1','B7'),('U4',44)))
require('Independent 5.1k Rd on each CC',net('J1','A5')!=net('J1','B5') and all(has_part('5.1k 1%',net('J1',p),'GND') for p in ['A5','B5']))
require('Input fuse precedes surge TVS and eFuse',same(('F1',2),('D5',1),('U14',5)) and net('F1',1)==net('J1','A4'))
require('Protected output powers ESD reference, charger and VBUS sensing',same(('U14',6),('D1',1),('U1',5),('U2',10),('Q1',1)) and net('U14',6)!=net('U14',5))
require('Guard UVLO divider and enable clamp are fitted',has_part('22.1k 1%',net('U14',5),net('U14',1)) and has_part('10k 1%',net('U14',1),'GND') and same(('D4',1),('U14',1)) and net('D4',2)=='GND')
require('Guard OVLO uses0.1percent divider independent of MCU',has_part('374k 0.1%',net('U14',5),net('U14',2)) and has_part('100k 0.1%',net('U14',2),'GND'))
require('Guard current and inrush controls have correct values',has_part('6.49k 1%',net('U14',9),'GND') and has_part('1nF C0G',net('U14',7),'GND'))
require('Guard fault is wired to dedicated MCU PC0',same(('U14',4),('U4',8)))
require('100mA 4.2V hardware charge selection',value('U2')=='BQ25185DLHR' and has_part('24k 1%',net('U2',7),'GND'))
require('100mA target charge current',has_part('3.00k 1%',net('U2',8),'GND'))
require('Cell and NTC separate',same(('U2',2),('J2',1)) and net('J2',1)!=net('J2',2) and same(('J2',2),('U2',6)))
require('Charge enable uses MOS drain; no SYS on GPIO',same(('U2',4),('Q2',3)) and same(('U4',56),('Q2',1)) and net('Q2',1)!=net('Q2',3))
require('Fixed 3.3V regulator feedback correct',same(('U3',1),('U3',10)))
require('Two H573 VCAP pins are separate',net('U4',30)!=net('U4',62) and all(has_part('2.2uF low ESR',net('U4',p),'GND') for p in [30,62]))
require('VCAP nets have only MCU and capacitors',all(all(ref=='U4' or ref.startswith('C') for ref,pin in ns[net('U4',p)]) for p in [30,62]))
require('SE VOUT only local VCC',same(('U5',15),('U5',18)) and net('U5',18)!=net('U5',12))
require('SE and LCD I2C buses distinct',net('U5',9)!=net('U8',3) and net('U5',10)!=net('U8',2))
require('SE I2C1 on PB6/PB7, not nonexistent PB9',same(('U5',10),('U4',58)) and same(('U5',9),('U4',59)))
require('LCD I2C3 PA8/PC9',same(('U8',2),('U4',41)) and same(('U8',3),('U4',40)))
require('LCD dedicated3.0V rail and flying capacitor',same(('U8',5),('U9',5)) and has_part('100nF',net('U8',7),net('U8',8)) and has_part('1uF',net('U8',6),net('U8',5)))
require('LCD white backlight current limiting and MOS gate',has_part('33R 1%',net('U8','A'),'+3V3') and same(('U8','K'),('Q3',3)) and same(('Q3',1),('U4',24)))
require('Fingerprint SPI pin order',same(('J5',5),('U4',53)) and same(('J5',6),('U4',52)) and same(('J5',7),('U4',51)))
require('Fingerprint SPI strap and bezel ground',same(('J5',1),('J5',11),('J5',12),('U4',18)))
require('Fingerprint supply not NFC harvested output',net('J5',10)!=net('U7',2))
require('NFC exposed pad uses ground and the documented package',net('U7',11)=='GND' and components['U7'].findtext('footprint')=='OpenKeys:FM11NC08S_DFN10_3x3_EP1.7x2.4')
require('NFC VOUT isolated from main/battery',net('U7',2) not in {net('U3',1),net('U2',1),net('J2',1)})
require('UV and OV directly drive reset',same(('U10',1),('U10',6),('U4',7),('U11',4),('U12',1)))
require('Supervisor monitors3.3V, powered from SYS',same(('U10',3),('U3',1),('U10',4)) and same(('U10',5),('U2',1)))
require('SE interlock cannot bypass reset',same(('U12',4),('U5',11)) and same(('U12',2),('U4',39)))
require('VBAT is isolated from switchable main3.3V',same(('U4',1),('U13',2)) and net('U4',1)!=net('U4',19) and same(('U13',3),('J2',1)))
require('Case contact uses backup3.0V and PC13',same(('SW6',1),('U4',1)) and same(('SW6',2),('U4',2),('U11',2)))
require('LSE connected to PC14/15',same(('Y1',1),('U4',3)) and same(('Y1',2),('U4',4)))
require('Each physical button has independent GPIO',len({net('SW'+str(n),1) for n in range(2,6)})==4)
erc=json.loads((R/'review/erc.json').read_text());v=[v for s in erc['sheets'] for v in s['violations']]
require('ERC zero electrical errors',not any(i['severity']=='error' for i in v))
expected={'J4'};warnings=[i for i in v if i['severity']=='warning'];found=set()
for w in warnings:
 require('Only documented RF footprint warnings',w['type']=='footprint_link_issues')
 for item in w['items']:
  m=re.search(r'Symbol (\w+) ',item['description'])
  if m:found.add(m.group(1))
require('Antenna footprint blocker retained',found==expected and len(warnings)==1)
report={'revision':'0.3','status':'ENGINEERING_CHECKS_PASS_WITH_RELEASE_BLOCKERS','checks':checks,'net_count':len(ns),'erc_errors':0,'erc_warnings':len(warnings),'unresolved_footprints':sorted(found),'manufacturing_release':False,'physical_validation':False}
(R/'review/netlist-check.json').write_text(json.dumps(report,indent=2)+'\n')
print(f'{len(checks)} checks passed; ERC0errors,1antenna footprint warning; NOT manufacturing release')
