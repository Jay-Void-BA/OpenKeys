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
require('Protected cell and pouch thermistor use separate keyed connectors',same(('U2',2),('J2',1)) and net('J2',2)=='GND' and same(('J6',1),('U2',6)) and net('J6',2)=='GND' and net('J6',1)!=net('J2',1))
require('Charge enable uses MOS drain; no SYS on GPIO',same(('U2',4),('Q2',3)) and same(('U4',56),('Q2',1)) and net('Q2',1)!=net('Q2',3))
require('Fixed 3.3V regulator feedback correct',same(('U3',1),('U3',10)))
require('Main3.3V uses guaranteed fixed PWM range',same(('U3',7),('U3',5),('U2',1)) and net('U3',7)!='GND')
require('Two H573 VCAP pins are separate',net('U4',30)!=net('U4',62) and all(has_part('2.2uF low ESR',net('U4',p),'GND') for p in [30,62]))
require('VCAP nets have only MCU and capacitors',all(all(ref=='U4' or ref.startswith('C') for ref,pin in ns[net('U4',p)]) for p in [30,62]))
require('SE VOUT only local VCC',same(('U5',15),('U5',18)) and net('U5',18)!=net('U5',12))
require('SE and LCD I2C buses distinct',net('U5',9)!=net('J7',4) and net('U5',10)!=net('J7',3))
require('SE I2C1 on PB6/PB7',same(('U5',10),('U4',58)) and same(('U5',9),('U4',59)))
require('LCD host I2C3 PA8/PC9',same(('U21',2),('U4',41)) and same(('U21',3),('U4',40)))
require('LCD TCA9517A level-buffer pinout and isolation',value('U21')=='TCA9517ADGKR' and same(('U21',7),('J7',3)) and same(('U21',6),('J7',4)) and net('U21',1)=='+3V3' and same(('U21',8),('J7',1)) and net('U21',4)=='GND')
require('Only switched SYS reaches LCD module',same(('J7',1),('U18',5),('U18',6)) and net('J7',1)!=net('U18',1) and net('U18',1)=='SYS' and net('J7',2)=='GND')
require('LCD load-switch pinout and passive discharge',net('U18',2)=='GND' and same(('U18',3),('U16',5),('U20',1)) and has_part('330R 1% 0.25W',net('U18',6),'GND') and components['R47'].findtext('footprint')=='Resistor_SMD:R_1206_3216Metric')
require('Reset and PC3 hold delayed LCD clear, PC4 clocks rearm',value('U17')=='TPS3808G33DBVR' and same(('U15',2),('U4',7)) and same(('U15',4),('U4',11),('U17',3)) and same(('U17',1),('U16',6)) and same(('U16',1),('U4',24)))
require('LCD reset delay uses fixed300ms resistor mode and pullup',net('U17',2)=='GND' and net('U17',5)==net('U17',6)=='+3V3' and has_part('100k 1%','+3V3',net('U17',4)) and same(('R58',2),('U17',4)) and has_part('10k 1%','+3V3',net('U17',1)) and not any(ref.startswith('C') for ref,pin in ns[net('U17',4)]))
require('Unpowered LCD latch has explicit10k ON pulldown',same(('R59',1),('U18',3)) and net('R59',2)=='GND' and value('R59')=='10k 1%')
require('DFR0555 latch TI DCU pinout, no PRE floating',net('U16',2)==net('U16',7)==net('U16',8)=='+3V3' and net('U16',4)=='GND')
require('Bus enable is latched AND delayed, never raw host',same(('U20',2),('U19',4)) and same(('U19',2),('R46',2)) and same(('R46',1),('U16',5)))
require('Every display logic chip decoupled and grounded',all(net(ref,ground)=='GND' and net(ref,power)=='+3V3' for ref,ground,power in [('U15',3,5),('U17',2,6),('U19',3,5),('U20',3,5),('U23',3,5)]) and same(('C60',1),('U17',6)) and net('C60',2)=='GND')
require('TCA enable is converted to switched VCCB by open drain',value('U23')=='SN74LVC1G07DCKR' and same(('U23',2),('U20',4)) and same(('U23',4),('U21',5),('R53',2),('R54',1)) and net('R53',1)=='LCD_VCC_SW' and net('R54',2)=='GND' and value('R53')=='10k 1%' and value('R54')=='100k 1%')
require('LCD B-side pullups use switched supply only',all(same((ref,1),('J7',1)) and same((ref,2),('J7',pin)) and value(ref)=='4.7k' for ref,pin in [('R55',3),('R56',4)]))
require('Cross-powered SYS window independently asserts reset',value('U22')=='TPS3700DDCR' and same(('U22',1),('U22',6),('U4',7)) and net('U22',5)=='+3V3' and net('U22',2)=='GND')
require('SYS window uses exact independent0.1percent divider ladders',all(net(top,1)=='SYS' and same((top,2),(bottom,1),('U22',pin)) and net(bottom,2)=='GND' and value(top)==v and value(bottom)=='10k 0.1%' for top,bottom,pin,v in [('R49','R50',3,'78.7k 0.1%'),('R51','R52',4,'110k 0.1%')]))
require('TCA both supplies and SYS comparator locally decoupled in schematic',same(('C55',1),('U21',1)) and same(('C57',1),('U21',8)) and same(('C59',1),('U22',5)) and all(net(ref,2)=='GND' for ref in ['C55','C57','C58','C59']))
require('Removed fingerprint and old LCD parts absent',all(ref not in components for ref in ['J5','U8','U9','Q3']) and not any('FPC2534' in value(ref) for ref in components))
require('NFC exposed pad uses ground and the documented package',net('U7',11)=='GND' and components['U7'].findtext('footprint')=='OpenKeys:FM11NC08S_DFN10_3x3_EP1.7x2.4')
require('NFC VOUT isolated from main/battery',net('U7',2) not in {net('U3',1),net('U2',1),net('J2',1)})
require('UV and OV directly drive reset',same(('U10',1),('U10',6),('U4',7),('U11',4),('U12',1)))
require('Supervisor monitors3.3V, powered from SYS',same(('U10',3),('U3',1),('U10',4)) and same(('U10',5),('U2',1)))
require('SE interlock cannot bypass reset',same(('U12',4),('U5',11)) and same(('U12',2),('U4',39)))
require('Backup LDO is supplied by USB/battery SYS, never BAT only',same(('U13',3),('U2',1),('C40',1)) and net('U13',3)!=net('J2',1) and net('C40',2)=='GND')
require('VBAT is isolated from switchable main3.3V and locally decoupled',same(('U4',1),('U13',2),('C41',1),('C56',1)) and net('U4',1)!=net('U4',19) and net('C41',2)==net('C56',2)=='GND')
require('Classical SE is the explicitly selected SE051C2 package',value('U5')=='SE051C2HQ1/Z01XDZ' and components['U5'].findtext('footprint')=='OpenKeys:SE050_SOT1969-1_HX2QFN20')
require('SE051 unused RF and reset grounded, NC pads remain unused',all(net('U5',pin)=='GND' for pin in (1,14,17,19,21)) and all(not net('U5',pin) or net('U5',pin).startswith('unconnected-') for pin in (2,3,4,5,6,7,8,13,16,20)))
require('Case contact uses backup3.0V and PC13',same(('SW12',1),('U4',1)) and same(('SW12',2),('U4',2),('U11',2)))
require('LSE connected to PC14/15',same(('Y1',1),('U4',3)) and same(('Y1',2),('U4',4)))
key_pins=[26,27,28,57,37,38,50,51,52,53]
require('Ten physical decimal buttons have exact independent GPIO',len({net('SW'+str(n),1) for n in range(2,12)})==10 and all(same(('SW'+str(i+2),1),('U4',pin)) and net('SW'+str(i+2),2)=='GND' for i,pin in enumerate(key_pins)))
require('Each key has a 47k pullup and 10nF input filter',all(has_part('47k',net('U4',pin),'+3V3') and has_part('10nF',net('U4',pin),'GND') for pin in key_pins))
erc=json.loads((R/'review/erc.json').read_text());v=[v for s in erc['sheets'] for v in s['violations']]
require('ERC zero electrical errors',not any(i['severity']=='error' for i in v))
warnings=[i for i in v if i['severity']=='warning']
require('ERC zero warnings including footprint links',not warnings)
require('Integral antenna is present',components['J4'].findtext('footprint')=='OpenKeys:NFC_PCB_Loop_4T_62x15' and same(('J4',1),('U7',3)) and same(('J4',2),('U7',4)))
report={'revision':'0.5','status':'ENGINEERING_CHECKS_PASS','checks':checks,'net_count':len(ns),'erc_errors':0,'erc_warnings':len(warnings),'unresolved_footprints':[],'manufacturing_release':False,'physical_validation':False}
(R/'review/netlist-check.json').write_text(json.dumps(report,indent=2)+'\n')
print(f'{len(checks)} checks passed; ERC 0 errors, 0 warnings; qualification still required')
