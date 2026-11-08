#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""Check safety-relevant connections from KiCad's exported XML, not generator state."""
from pathlib import Path
import xml.etree.ElementTree as ET
import json,re,datetime
R=Path(__file__).resolve().parents[1]
x=ET.parse(R/'exports/OpenKeys-netlist.xml').getroot()
net_nodes={}
for n in x.findall('./nets/net'):
 net_nodes[n.attrib['name']]={(a.attrib['ref'],a.attrib['pin']) for a in n.findall('node')}
checks=[]
def require(name,condition):
 if not condition:raise SystemExit('FAILED: '+name)
 checks.append(name)
def net(ref,pin):
 ns=[name for name,nodes in net_nodes.items() if (ref,str(pin)) in nodes]
 if len(ns)!=1:raise SystemExit(f'{ref}.{pin}: expected one net, got {ns}')
 return ns[0]
def together(*p):return len({net(*a) for a in p})==1
require('USB-A and USB-C raw VBUS are separate',net('J1',1)!=net('J2','A4'))
require('USB-A and USB-C D+ are separate until data mux',net('J1',3)!=net('J2','A6'))
require('USB-A and USB-C D- are separate until data mux',net('J1',2)!=net('J2','A7'))
require('USB-C duplicated D+ pads connected',together(('J2','A6'),('J2','B6')))
require('USB-C duplicated D- pads connected',together(('J2','A7'),('J2','B7')))
require('USB-C CC1 and CC2 have independent nets',net('J2','A5')!=net('J2','B5'))
require('Selected data connects to MCU USB D+',together(('U1',4),('U7',45)))
require('Selected data connects to MCU USB D-',together(('U1',6),('U7',44)))
require('MCU USB is isolated from each raw port by mux',net('U7',45) not in {net('J1',3),net('J2','A6')})
require('SE output powers its VCC only',together(('U8',15),('U8',18)) and net('U8',12)!=net('U8',18))
require('SE input uses the main regulated supply',together(('U8',12),('U6',1)))
require('SE I2C separate from display/RTC I2C',net('U8',9)!=net('U10',4) and net('U8',10)!=net('U10',3))
require('Fixed 3.3V regulator FB connects to VOUT',together(('U6',10),('U6',1)))
require('MCU VCAP has no external power loads',net('U7',30) not in {net('U6',1),net('U5',10),net('J3',1)})
require('NFC VOUT never powers main supply or battery',net('U11',2) not in {net('U6',1),net('U5',10),net('J3',1)})
require('Battery positive and NTC are distinct',net('J3',1)!=net('J3',2))
require('MCU charge-enable controls MOS gate, not SYS voltage',together(('U7',56),('Q3',1)) and net('Q3',1)!=net('Q3',3))
require('Charging CE controlled by open-drain MOS drain',together(('U5',4),('Q3',3)))
require('USB current controls default to distinct signals',net('U5',6)!=net('U5',5))
require('LQFP64 SPI2 MISO uses pin 10 (PC2)',together(('U7',10),('U9',2)))
require('LQFP64 I2C2 SDA uses pin 35 (PB14)',together(('U7',35),('U10',4)))
# Reject any ERC error or warning except the eight deliberately unresolved footprints.
erc=json.loads((R/'review/erc.json').read_text());viol=[v for s in erc['sheets'] for v in s['violations']]
errors=[v for v in viol if v['severity']=='error'];warnings=[v for v in viol if v['severity']=='warning']
require('ERC has zero electrical errors',not errors)
expected={'J1','J2','J5','J7','U8','U11','SW1','SW6'};got=set()
for v in warnings:
 require('Only documented footprint warnings remain: '+v['description'],v['type']=='footprint_link_issues')
 for i in v['items']:
  m=re.search(r'Symbol (\w+) ',i['description'])
  if m:got.add(m.group(1))
require('Exactly eight unresolved footprint references are documented',got==expected and len(warnings)==8)
report={'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'source':'KiCad XML netlist + unmodified ERC JSON','result':'PASS_WITH_RELEASE_BLOCKERS','checks':checks,'net_count':len(net_nodes),'erc_errors':len(errors),'erc_warnings':len(warnings),'unresolved_footprints':sorted(got),'manufacturing_release':False,'physical_validation':False}
(R/'review/netlist-check.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(f'{len(checks)} checks passed; {len(errors)} ERC errors, {len(warnings)} unresolved footprint warnings. NOT fabrication-ready.')
