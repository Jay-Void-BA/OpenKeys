#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""Reproducible editable KiCad 9 schematic; no third-party Python modules."""
from pathlib import Path
import copy, json, re, uuid, math, csv
ROOT=Path(__file__).resolve().parents[1]
CAD=ROOT/'kicad'
class Q(str): pass

def parse(s):
    ts=re.findall(r'"(?:\\.|[^"\\])*"|[()]|[^\s()]+',s)
    i=0
    def read():
        nonlocal i
        t=ts[i];i+=1
        if t=='(':
            a=[]
            while ts[i]!=')':a.append(read())
            i+=1;return a
        return Q(json.loads(t)) if t.startswith('"') else t
    return read()
def dump(a):
    if isinstance(a,list): return '('+' '.join(dump(x) for x in a)+')'
    if isinstance(a,Q): return json.dumps(str(a),ensure_ascii=False)
    return str(a)
def pretty(a,level=0):
    if not isinstance(a,list) or not any(isinstance(x,list) for x in a):return dump(a)
    split=next(i for i,x in enumerate(a) if isinstance(x,list))
    prefix='('+' '.join(dump(x) for x in a[:split])
    return prefix+'\n'+'\n'.join('  '*(level+1)+pretty(x,level+1) for x in a[split:])+'\n'+'  '*level+')'
def nodes(a,k): return [x for x in a if isinstance(x,list) and x and x[0]==k]
def node(a,k): return next((x for x in a if isinstance(x,list) and x and x[0]==k),None)
def nuid(key): return str(uuid.uuid5(uuid.NAMESPACE_URL,'https://github.com/Jay-Void-BA/OpenKeys/r0/'+key))
def q(s): return Q(str(s))
def effects(size=1.0,hide=False):
    return ['effects',['font',['size',size,size]],*([['hide','yes']] if hide else [])]
def library(lib,name):
    doc=parse(Path('/usr/share/kicad/symbols',lib+'.kicad_sym').read_text())
    data=next((x for x in nodes(doc,'symbol') if x[1]==name),None)
    if data is None:raise ValueError((lib,name))
    def flatten(a):
        ext=node(a,'extends')
        if ext:
            par=flatten(next(x for x in nodes(doc,'symbol') if x[1]==ext[1]))
            props={x[1]:x for x in nodes(par,'property')}
            props.update({x[1]:x for x in nodes(a,'property')})
            rest=[x for x in par[2:] if not(isinstance(x,list) and x[0] in ('property','extends'))]
            for item in rest:
                if isinstance(item,list) and item[0]=='symbol': item[1]=q(str(item[1]).replace(str(par[1])+'_',str(a[1])+'_',1))
            return ['symbol',a[1],*props.values(),*rest]
        return copy.deepcopy(a)
    out=flatten(data);out[1]=q(name)
    return out

def custom(name,pins):
    height=math.ceil(len(pins)/2)*2.54/2+2.54
    a=['symbol',q(name),['pin_names',['offset',0.7]],['in_bom','yes'],['on_board','yes'],
       ['property',q('Reference'),q('U'),['at',0,height+3,0],effects()],
       ['property',q('Value'),q(name),['at',0,height+1,0],effects()],
       ['symbol',q(name+'_0_1'),['rectangle',['start',-10.16,height],['end',10.16,-height],['stroke',['width',0.254],['type','default']],['fill',['type','background']]]]]
    inner=['symbol',q(name+'_1_1')]
    half=math.ceil(len(pins)/2)
    for idx,(num,label,typ) in enumerate(pins):
        side=0 if idx<half else 1;j=idx%half
        inner.append(['pin',typ,'line',['at',-12.7 if side==0 else 12.7,round(height-2.54-j*2.54,4),0 if side==0 else 180],['length',2.54],['name',q(label),effects()],['number',q(num),effects()]])
    a.append(inner);return a
SPECS={
'REG':('Regulator_Switching','TPS63031DSK'),
'USBC':('Connector','USB_C_Receptacle_USB2.0_16P'),
'ESD':('Power_Protection','USBLC6-2SC6'),
'FLASH':('Memory_Flash','W25Q32JVSS'),
'R':('Device','R'),'C':('Device','C'),'L':('Device','L'),
'FB':('Device','FerriteBead'),'FUSE':('Device','Polyfuse'),
'NMOS':('Transistor_FET','2N7002'),
'SW':('Switch','SW_Push'),'SPDT':('Switch','SW_SPDT'),
'CONN2':('Connector_Generic','Conn_01x02'),'CONN3':('Connector_Generic','Conn_01x03'),
'CONN4':('Connector_Generic','Conn_01x04'),'CONN12':('Connector_Generic','Conn_01x12'),'CONN10':('Connector_Generic','Conn_02x05_Odd_Even'),
'TVS':('Device','D_TVS'),'PWR':('power','PWR_FLAG')}
LIB={k:library(*v) for k,v in SPECS.items()}
LIB['SE']=custom('SE051C2',[(1,'LB','passive'),(2,'NC','no_connect'),(3,'IO1','bidirectional'),(4,'NC','no_connect'),(5,'NC','no_connect'),(6,'NC','no_connect'),(7,'NC','no_connect'),(8,'NC','no_connect'),(9,'SDA','bidirectional'),(10,'SCL','input'),(11,'ENA','input'),(12,'VIN','power_in'),(13,'CLK','input'),(14,'RST_N','input'),(15,'VOUT','power_out'),(16,'IO2','bidirectional'),(17,'LA','passive'),(18,'VCC','power_in'),(19,'VSS','power_in'),(20,'NC','no_connect'),(21,'EP','passive')])
LIB['NFC']=custom('FM11NC08S',[(1,'VCC','power_in'),(2,'VOUT','power_out'),(3,'LC1','passive'),(4,'LC2','passive'),(5,'VSS','power_in'),(6,'SSN','input'),(7,'SCLK','input'),(8,'MOSI','input'),(9,'MISO','tri_state'),(10,'IRQ_N','open_collector'),(11,'EP','passive')])
def pins(lib):
    return [x for unit in nodes(lib,'symbol') for x in nodes(unit,'pin')]
def pmap(k):return {str(node(p,'number')[1]):str(node(p,'name')[1]) for p in pins(LIB[k])}
class Sheet:
    def __init__(self,name,title,index):
        self.name,self.index=name,index;self.uid=nuid(name);self.items=[];self.used={};self.components=[]
        self.items.append(['text',q(title),['at',205,16,0],effects(2.5),['uuid',q(nuid(name+'/title'))]])
    def text(self,s,x,y,size=1.3): self.items.append(['text',q(s),['at',x,y,0],effects(size),['uuid',q(nuid(self.name+s+str(x)+str(y)))]])
    def comp(self,k,ref,value,x,y,nets,footprint='',note='',dnp=False):
        lib=LIB[k];id='OpenKeys:'+str(lib[1]);self.used[id]=lib
        pp=pins(lib);top=min(-float(node(p,'at')[2]) for p in pp);bottom=max(-float(node(p,'at')[2]) for p in pp)
        sym=['symbol',['lib_id',q(id)],['at',x,y,0],['unit',1],['in_bom','no' if k=='PWR' else 'yes'],['on_board','no' if k=='PWR' else 'yes'],['dnp','yes' if dnp else 'no'],['uuid',q(nuid(ref))]]
        fp=footprint or next((str(a[2]) for a in nodes(lib,'property') if a[1]=='Footprint'),'')
        for pname,val,py,hide in [('Reference',ref,y+top-4,False),('Value',value,y+top-2,False),('Footprint',fp,y,True),('Datasheet',next((str(a[2]) for a in nodes(lib,'property') if a[1]=='Datasheet'),''),y,True)]:
            sym.append(['property',q(pname),q(val),['at',round((x+4.0 if k in ('R','C','L','FB','FUSE','TVS','NMOS','SW','SPDT') else x-20.32 if k in ('MCU','PMUX','CHG','REG','DMUX','FLASH','RTC','USBA','USBC') else x),4),round((y+(-1.5 if pname=='Reference' else 1.5) if k in ('R','C','L','FB','FUSE','TVS','NMOS','SW','SPDT') and not hide else py),4),0],[*effects(1,hide),*([['justify','left']] if k in ('R','C','L','FB','FUSE','TVS','NMOS','SW','SPDT') and not hide else [])]])
        sym.append(['instances',['project',q('OpenKeys'),['path',q('/'+nuid('OpenKeys')+'/'+self.uid),['reference',q(ref)],['unit',1]]]])
        actual={str(node(p,'number')[1]) for p in pp}
        assert set(map(str,nets))<=actual,(ref,set(nets)-actual)
        for p in pp:
            num=str(node(p,'number')[1]);at=node(p,'at');px=round(x+float(at[1]),4);py=round(y-float(at[2]),4);angle=int(at[3])
            sym.append(['pin',q(num),['uuid',q(nuid(ref+'/'+num))]])
            net=nets.get(num)
            if net is None:
                self.items.append(['no_connect',['at',px,py],['uuid',q(nuid(ref+'/'+num+'/nc'))]])
            else:
                dx,dy={0:(-5.08,0),180:(5.08,0),90:(0,5.08),270:(0,-5.08)}[angle]
                ex,ey=round(px+dx,4),round(py+dy,4)
                self.items.append(['wire',['pts',['xy',px,py],['xy',ex,ey]],['stroke',['width',0],['type','default']],['uuid',q(nuid(ref+'/'+num+'/wire'))]])
                la={0:180,180:0,90:270,270:90}[angle]
                # Local orientation mirrors KiCad power-label insertion behavior.
                self.items.append(['global_label',q(net),['shape','input'],['at',ex,ey,la],[*effects(0.9),['justify','left' if la in (0,90) else 'right']],['uuid',q(nuid(ref+'/'+num+'/label'))],['property',q('Intersheetrefs'),q('${INTERSHEET_REFS}'),['at',ex,ey,la],effects(0.8,True)]])
        self.items.append(sym)
        if k!='PWR':self.components.append(dict(ref=ref,value=value,footprint=fp,sheet=self.name,nets=nets,note=note,dnp=dnp))
    def save(self):
        libs=[]
        for id,lib in self.used.items():l=copy.deepcopy(lib);l[1]=q(id);libs.append(l)
        doc=['kicad_sch',['version',20250114],['generator',q('eeschema')],['uuid',q(self.uid)],['paper',q('A3')],['title_block',['title',q('OpenKeys R0.5 - '+self.name)],['date',q('2026-11-13')],['rev',q('0.5-board-review')],['company',q('Jay-Void-BA')],['comment',1,q('ENGINEERING BOARD CANDIDATE - qualify before use')]],['lib_symbols',*libs],*self.items]
        (CAD/(self.name+'.kicad_sch')).write_text(pretty(doc)+'\n')

S=[]
def sheet(name,title):
    s=Sheet(name,title,len(S)+2);S.append(s);return s
refcounts={'R':0,'C':0,'L':0,'FB':0,'F':0,'Q':0,'SW':0,'J':0,'U':0}
def ref(t):
    existing={c['ref'] for ss in S for c in ss.components}
    while True:
        refcounts[t]+=1;candidate=t+str(refcounts[t])
        if candidate not in existing:return candidate
def passive(s,k,val,x,y,a,b,note='',dnp=False):
    prefix={'R':'R','C':'C','L':'L','FB':'FB','FUSE':'F'}[k]
    fp={'R':'Resistor_SMD:R_0603_1608Metric','C':'Capacitor_SMD:C_0603_1608Metric','L':'Inductor_SMD:L_Taiyo-Yuden_MD-3030','FB':'Inductor_SMD:L_0603_1608Metric','FUSE':'Fuse:Fuse_1206_3216Metric'}[k]
    if k=='C' and ('uF' in val or '1kV' in val):
        fp='Capacitor_SMD:C_0805_2012Metric'
    if k=='R' and '0.25W' in val:
        fp='Resistor_SMD:R_1206_3216Metric'
    if k=='C' and '2kV' in val:
        fp='Capacitor_SMD:C_1206_3216Metric'
    if k=='L':
        fp='Inductor_SMD:L_Sunlord_SWPA3015S'
    s.comp(k,ref(prefix),val,x,y,{'1':a,'2':b},fp,note,dnp)
def caps(s,values,x,y):
    for i,(val,a,b) in enumerate(values):passive(s,'C',val,x+(i%9)*33.02,y+(i//9)*33.02,a,b)
