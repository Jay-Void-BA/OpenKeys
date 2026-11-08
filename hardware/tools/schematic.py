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
'MCU':('MCU_ST_STM32U5','STM32U585RITx'),
'PMUX':('Power_Management','TPS2116DRL'),
'CHG':('Battery_Management','BQ24074RGT'),
'REG':('Regulator_Switching','TPS63031DSK'),
'DMUX':('Interface_USB','TS3USB30EDGSR'),
'USBC':('Connector','USB_C_Receptacle_USB2.0_16P'),
'USBA':('Connector','USB_A'),
'ESD':('Power_Protection','USBLC6-2SC6'),
'FLASH':('Memory_Flash','W25Q32JVSS'),
'RTC':('Timer_RTC','RV-3028-C7'),
'R':('Device','R'),'C':('Device','C'),'L':('Device','L'),
'FB':('Device','FerriteBead'),'FUSE':('Device','Polyfuse'),
'NMOS':('Transistor_FET','2N7002'),
'SW':('Switch','SW_Push'),'SPDT':('Switch','SW_SPDT'),
'CONN2':('Connector_Generic','Conn_01x02'),'CONN3':('Connector_Generic','Conn_01x03'),
'CONN4':('Connector_Generic','Conn_01x04'),'CONN10':('Connector_Generic','Conn_02x05_Odd_Even'),
'TVS':('Device','D_TVS'),'PWR':('power','PWR_FLAG')}
LIB={k:library(*v) for k,v in SPECS.items()}
LIB['SE']=custom('SE050E',[(1,'LB','passive'),(2,'NC','no_connect'),(3,'IO1','bidirectional'),(4,'NC','no_connect'),(5,'NC','no_connect'),(6,'NC','no_connect'),(7,'NC','no_connect'),(8,'NC','no_connect'),(9,'SDA','bidirectional'),(10,'SCL','input'),(11,'ENA','input'),(12,'VIN','power_in'),(13,'CLK','input'),(14,'RST_N','input'),(15,'VOUT','power_out'),(16,'IO2','bidirectional'),(17,'LA','passive'),(18,'VCC','power_in'),(19,'VSS','power_in'),(20,'NC','no_connect'),(21,'EP','passive')])
LIB['NFC']=custom('FM11NC08S',[(1,'VCC','power_in'),(2,'VOUT','power_out'),(3,'LC1','passive'),(4,'LC2','passive'),(5,'VSS','power_in'),(6,'SSN','input'),(7,'SCLK','input'),(8,'MOSI','input'),(9,'MISO','tri_state'),(10,'IRQ_N','open_collector'),(11,'EP','passive')])
def pins(lib):
    return [x for unit in nodes(lib,'symbol') for x in nodes(unit,'pin')]
def pmap(k):return {str(node(p,'number')[1]):str(node(p,'name')[1]) for p in pins(LIB[k])}
if __name__=='__main__' and '--pins' in __import__('sys').argv:
    for k in ('MCU','PMUX','CHG','REG','DMUX','USBA','USBC','ESD','FLASH','RTC','NMOS','SPDT'):print(k,pmap(k))
    raise SystemExit
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
        doc=['kicad_sch',['version',20250114],['generator',q('eeschema')],['uuid',q(self.uid)],['paper',q('A3')],['title_block',['title',q('OpenKeys R0 - '+self.name)],['date',q('2026-11-08')],['rev',q('0.1-design-review')],['company',q('Jay-Void-BA')],['comment',1,q('NOT FOR FABRICATION - see hardware/RELEASE_GATES.md')]],['lib_symbols',*libs],*self.items]
        (CAD/(self.name+'.kicad_sch')).write_text(pretty(doc)+'\n')

S=[]
def sheet(name,title):
    s=Sheet(name,title,len(S)+2);S.append(s);return s
refcounts={'R':0,'C':0,'L':0,'FB':0,'F':0,'Q':0,'SW':0,'J':0,'U':0}
def ref(t):refcounts[t]+=1;return t+str(refcounts[t])
def passive(s,k,val,x,y,a,b,note='',dnp=False):
    prefix={'R':'R','C':'C','L':'L','FB':'FB','FUSE':'F'}[k]
    fp={'R':'Resistor_SMD:R_0603_1608Metric','C':'Capacitor_SMD:C_0603_1608Metric','L':'Inductor_SMD:L_Taiyo-Yuden_MD-3030','FB':'Inductor_SMD:L_0603_1608Metric','FUSE':'Fuse:Fuse_1206_3216Metric'}[k]
    s.comp(k,ref(prefix),val,x,y,{'1':a,'2':b},fp,note,dnp)
def caps(s,values,x,y):
    for i,(val,a,b) in enumerate(values):passive(s,'C',val,x+(i%9)*33.02,y+(i//9)*33.02,a,b)

def generate():
    # USB: each raw 5 V input has independent ESD protection and resettable fuse.
    s=sheet('01-usb','USB-A / USB-C - USB 2.0 FS, sink only')
    s.comp('USBA','J1','USB-A male (plug)',48.26,66.04,{'1':'VBUS_A_RAW','2':'USB_A_DM','3':'USB_A_DP','4':'GND','5':'SHIELD'},'OpenKeys:USB_A_Plug_THT_PENDING',note='Exact plug mechanical footprint is a fabrication blocker')
    s.comp('USBC','J2','USB-C USB2 receptacle',137.16,71.12,{'A1':'GND','A4':'VBUS_C_RAW','A5':'CC1','A6':'USB_C_DP','A7':'USB_C_DM','A8':None,'A9':'VBUS_C_RAW','A12':'GND','B1':'GND','B4':'VBUS_C_RAW','B5':'CC2','B6':'USB_C_DP','B7':'USB_C_DM','B8':None,'B9':'VBUS_C_RAW','B12':'GND','S1':'SHIELD'},'OpenKeys:USB_C_16P_RECEPTACLE_PENDING',note='Select exact connector and manufacturer drawing before PCB')
    def named(k,d):return {num:d.get(name) for num,name in pmap(k).items()}
    s.comp('DMUX','U1','TS3USB30EDGSR',266.7,66.04,named('DMUX',{'VCC':'+3V3','GND':'GND','D+':'USB_DP','D-':'USB_DM','D1+':'USB_A_DP','D1-':'USB_A_DM','D2+':'USB_C_DP','D2-':'USB_C_DM','S':'USB_SEL','~{OE}':'USB_OE_N'}))
    s.comp('ESD','U2','USBLC6-2SC6',55.88,134.62,{'1':'USB_A_DP','6':'USB_A_DP','3':'USB_A_DM','4':'USB_A_DM','2':'GND','5':'VBUS_A_RAW'})
    s.comp('ESD','U3','USBLC6-2SC6',137.16,134.62,{'1':'USB_C_DP','6':'USB_C_DP','3':'USB_C_DM','4':'USB_C_DM','2':'GND','5':'VBUS_C_RAW'})
    passive(s,'R','5.1k 1%',205.74,137.16,'CC1','GND');passive(s,'R','5.1k 1%',238.76,137.16,'CC2','GND')
    passive(s,'R','100k',271.78,137.16,'+3V3','USB_OE_N');passive(s,'R','100k',304.8,137.16,'USB_SEL','GND')
    passive(s,'R','1M',337.82,137.16,'SHIELD','GND');passive(s,'C','1nF / 2kV',370.84,137.16,'SHIELD','GND')
    for idx,(raw,sense,x) in enumerate([('VBUS_A_RAW','VBUS_A_N',63.5),('VBUS_C_RAW','VBUS_C_N',154.94)]):
        s.comp('NMOS','Q'+str(idx+1),'2N7002',x,200.66,{'1':raw,'2':'GND','3':sense})
        passive(s,'R','100k',x+33.02,195.58,'+3V3',sense)
        passive(s,'R','1M',x+33.02,227.33,raw,'GND')
    caps(s,[('100nF','+3V3','GND')],254,200.66)
    s.text('USB_SEL: 0=A, 1=C. /OE pulled HIGH disables all data at reset.\nFirmware only enables after selected VBUS is valid; stop on disconnect/port change.\nBoth plugs must never join D+/D- directly. No USB PD and no source VBUS.',270,239)
    s.text('CC and VBUS TVS fitted; verify leakage and clamp coordination.\nUSB-A plug and USB-C footprints remain unselected.\nGND is common; simultaneous hosts are not galvanically isolated.',92,250)
    for i,(net,x) in enumerate([('VBUS_A_RAW',35.56),('VBUS_C_RAW',101.6),('CC1',167.64),('CC2',233.68)]):
        s.comp('TVS','D'+str(i+1),'SMF5.0A' if i<2 else 'PESD5V0S1BA',x,270.51,{'1':net,'2':'GND'},'Diode_SMD:D_SOD-123F' if i<2 else 'Diode_SMD:D_SOD-323')
    for i,net in enumerate(['VBUS_A_RAW','VBUS_C_RAW','GND']):s.comp('PWR','#FLG'+str(i+1),'PWR_FLAG',320.04+i*30.48,31.75,{'1':net})
    # power
    s=sheet('02-power','USB power OR-ing, protected 1S battery, 3.3 V supply')
    passive(s,'FUSE','250mA hold - verify trip',33.02,58.42,'VBUS_A_RAW','VBUS_A')
    passive(s,'FUSE','250mA hold - verify trip',66.04,58.42,'VBUS_C_RAW','VBUS_C')
    s.comp('PMUX','U4','TPS2116DRL',137.16,58.42,named('PMUX',{'VIN1':'VBUS_A','VIN2':'VBUS_C','VOUT':'USB_5V','GND':'GND','MODE':'VBUS_A','PR1':'MUX_PR1','ST':'MUX_ST'}))
    passive(s,'R','300k 1%',101.6,119.38,'VBUS_A','MUX_PR1');passive(s,'R','100k 1%',134.62,119.38,'MUX_PR1','GND')
    # PR1 nominal 4 V transition; 1 V reference tolerance means 3.68..4.32 V.
    passive(s,'R','100k',167.64,119.38,'+3V3','MUX_ST')
    s.comp('CHG','U5','BQ24074RGT',243.84,63.5,named('CHG',{'IN':'USB_5V','OUT':'SYS','BAT':'BAT','VSS':'GND','EP':'GND','TS':'BAT_NTC','~{CE}':'CHARGE_DISABLE','CE':'CHARGE_DISABLE','EN1':'USB_LIMIT_500','EN2':'USB_LIMIT_EXT','~{CHG}':'CHG_N','~{PGOOD}':'PGOOD_N','ILIM':'CHG_ILIM','ISET':'CHG_ISET','ITERM':None,'TMR':None}))
    s.comp('CONN3','J3','Protected 1S 200mAh + 10k NTC',350.52,60.96,{'1':'BAT','2':'BAT_NTC','3':'GND'},'Connector_JST:JST_PH_S3B-PH-K_1x03_P2.00mm_Horizontal',note='NTC must be in cell pack; protection must include over/under-voltage and overcurrent')
    passive(s,'R','8.87k 1%',205.74,127,'CHG_ISET','GND',note='About 100 mA fast-charge; verify actual pack rating')
    passive(s,'R','7.5k 1%',238.76,127,'CHG_ILIM','GND')
    passive(s,'R','100k',271.78,127,'SYS','CHARGE_DISABLE')
    s.comp('NMOS','Q3','2N7002',365.76,99.06,{'1':'CHARGE_ENABLE','2':'GND','3':'CHARGE_DISABLE'})
    passive(s,'R','100k',370.84,139.7,'CHARGE_ENABLE','GND')
    s.comp('PWR','#FLG4','PWR_FLAG',350.52,27.94,{'1':'VBUS_A'})
    s.comp('PWR','#FLG8','PWR_FLAG',381,27.94,{'1':'VBUS_C'})
    passive(s,'R','100k',304.8,127,'USB_LIMIT_500','GND');passive(s,'R','100k',337.82,127,'USB_LIMIT_EXT','GND')
    caps(s,[('1uF 10V','VBUS_A','GND'),('1uF 10V','VBUS_C','GND'),('4.7uF 10V','USB_5V','GND'),('10uF 6.3V','BAT','GND'),('10uF 10V','SYS','GND')],40.64,172.72)
    # Enable regulator with physical OFF/ON; RTC remains on backup.
    s.comp('SPDT','SW1','OFF / ON',50.8,238.76,{'1':'SYS','2':'REG_EN','3':'GND'},'OpenKeys:SLIDE_SPDT_PENDING')
    s.comp('REG','U6','TPS63031DSK (3.3 V)',167.64,236.22,named('REG',{'VIN':'SYS','VINA':'REG_VINA','VOUT':'+3V3','FB':'+3V3','EN':'REG_EN','PS/SYNC':'GND','L1':'SW_L1','L2':'SW_L2','GND':'GND','PGND':'GND','EP':'GND'}))
    passive(s,'R','0R',205.74,185.42,'SYS','REG_VINA')
    passive(s,'L','1.5uH Isat>=1.5A',243.84,215.9,'SW_L1','SW_L2')
    caps(s,[('10uF 10V','SYS','GND'),('100nF','REG_VINA','GND'),('10uF 6.3V','+3V3','GND'),('10uF 6.3V','+3V3','GND')],269.24,220.98)
    s.text('Default USB100: EN2=0, EN1=0. Firmware must handle USB suspend/current limits.\nCharge is disabled until MCU policy checks pack, input budget and temperature.\nNo arbitrary LiPo substitution. Do not connect a bare unprotected cell.',290,177)
    # MCU
    s=sheet('03-mcu','STM32U585RIT6 - trusted I/O and protected runtime boundary')
    pn={v:k for k,v in pmap('MCU').items()}
    nets={}
    for num,name in pmap('MCU').items():
        if name in ('VDD','VDDA','VDDUSB','VBAT'):nets[num]='+3V3' if name!='VDDA' else 'VDDA'
        elif name in ('VSS','VSSA'):nets[num]='GND'
    signals={'NRST':'NRST','PH3':'BOOT0','VCAP':'VCAP','PA0':'USB_SEL','PA1':'USB_OE_N','PA2':'PGOOD_N','PA3':'CHG_N','PA4':'NFC_CS_N','PA5':'NFC_SCK','PA6':'NFC_MISO','PA7':'NFC_MOSI','PA11':'USB_DM','PA12':'USB_DP','PA13':'SWDIO','PA14':'SWCLK','PA15':'FP_CS_N','PB0':'BTN_UP_N','PB1':'BTN_DOWN_N','PB2':'BTN_OK_N','PB3':'SWO','PB4':'CHARGE_ENABLE','PB5':'BTN_CANCEL_N','PB6':'USB_LIMIT_500','PB7':'USB_LIMIT_EXT','PB8':'SE_SCL','PB9':'SE_SDA','PB10':'UI_SCL','PB12':'FLASH_CS_N','PB13':'FLASH_SCK','PB14':'UI_SDA','PB15':'FLASH_MOSI','PC2':'FLASH_MISO','PC3':'RTC_INT_N','PC4':'VBUS_A_N','PC5':'VBUS_C_N','PC6':'NFC_IRQ_N','PC7':'FP_RESET_N','PC8':'SE_EN','PC9':'FP_IRQ_N','PC10':'FP_SCK','PC11':'FP_MISO','PC12':'FP_MOSI','PC13':'CASE_TAMPER_N'}
    for pin,net in signals.items():nets[pn[pin]]=net
    s.comp('MCU','U7','STM32U585RIT6',160.02,142.24,nets,note='No PB11 on LQFP64; PC2=SPI2_MISO AF5, PB14=I2C2_SDA AF4')
    caps(s,[('100nF','+3V3','GND')]*4+[('4.7uF','+3V3','GND'),('4.7uF low ESR','VCAP','GND'),('100nF','VDDA','GND'),('1uF','VDDA','GND')],35.56,35.56)
    passive(s,'FB','600R@100MHz',309.88,86.36,'+3V3','VDDA')
    passive(s,'R','10k',309.88,119.38,'+3V3','NRST');passive(s,'C','100nF',342.9,119.38,'NRST','GND')
    passive(s,'R','10k',309.88,152.4,'BOOT0','GND')
    s.comp('CONN10','J4','Cortex SWD - DNP production',312.42,218.44,{'1':'+3V3','2':'SWDIO','3':'GND','4':'SWCLK','5':'GND','6':'SWO','7':None,'8':None,'9':'GND','10':'NRST'},'Connector_PinHeader_1.27mm:PinHeader_2x05_P1.27mm_Vertical',dnp=True)
    s.text('HSI48 + CRS for USB; no external HSE fitted. Validate clock accuracy over PVT.\nVDDA filter and VCAP are NOT optional; VCAP must not supply external loads.\nSecurity requires signed boot/update, monotonic rollback state and debug lifecycle.\nThese schematic nets do not configure or prove those protections.',126,255)
    s.text('Pinmux assignments are listed in pinout.csv.\nScreen/buttons belong to trusted device code.\nPC13: tamper event locks + latches.\nNo automatic wipe on switch bounce.',320,58)
    # secure + RTC + flash
    s=sheet('04-se-storage-rtc','Secure element, encrypted external storage and timekeeping')
    s.comp('SE','U8','SE050E2HQ1/Z01Z3Z',71.12,83.82,{'1':'GND','9':'SE_SDA','10':'SE_SCL','11':'SE_EN','12':'+3V3','14':'GND','15':'SE_CORE','17':'GND','18':'SE_CORE','19':'GND','21':'GND'},'OpenKeys:SE050_HVQFN20_PENDING',note='Deep-power-down topology. VOUT powers VCC, never connect VOUT to +3V3')
    s.comp('FLASH','U9','W25Q64JVSSIQ',205.74,81.28,named('FLASH',{'VCC':'+3V3','GND':'GND','~{CS}':'FLASH_CS_N','CLK':'FLASH_SCK','DI':'FLASH_MOSI','DO':'FLASH_MISO','DI/IO_{0}':'FLASH_MOSI','DO/IO_{1}':'FLASH_MISO','~{WP}/IO_{2}':'FLASH_WP_N','~{HOLD}/~{RESET}/IO_{3}':'FLASH_HOLD_N','~{WP}':'+3V3','~{HOLD}':'+3V3','IO0':'FLASH_MOSI','IO1':'FLASH_MISO','IO2':'+3V3','IO3':'+3V3'}))
    s.comp('RTC','U10','RV-3028-C7',330.2,81.28,named('RTC',{'VDD':'+3V3','VSS':'GND','GND':'GND','VBACKUP':'BAT','SCL':'UI_SCL','SDA':'UI_SDA','~{INT}':'RTC_INT_N','INT':'RTC_INT_N','CLKOUT':None,'EVI': 'GND'}))
    for i,(a,b,v) in enumerate([('+3V3','SE_SCL','4.7k'),('+3V3','SE_SDA','4.7k'),('SE_EN','GND','100k'),('+3V3','FLASH_CS_N','47k'),('+3V3','UI_SCL','4.7k'),('+3V3','UI_SDA','4.7k'),('+3V3','RTC_INT_N','47k'),('+3V3','CHG_N','47k'),('+3V3','PGOOD_N','47k')]):passive(s,'R',v,35.56+i*38.1,154.94,a,b)
    passive(s,'R','47k',264.16,215.9,'+3V3','FLASH_WP_N')
    passive(s,'R','47k',302.26,215.9,'+3V3','FLASH_HOLD_N')
    caps(s,[('100nF','+3V3','GND'),('100nF','SE_CORE','GND'),('1uF','SE_CORE','GND'),('100nF','+3V3','GND'),('100nF','+3V3','GND')],50.8,215.9)
    s.text('SE I2C1 is separate from UI I2C2. Use authenticated SE session and policy-bound objects.\nExternal flash stores AEAD ciphertext with authenticated metadata, version and anti-rollback design.\nSE certification alone does not validate MCU, fingerprint path, PCB or complete product.',134,255)
    s.text('RTC is a time source, not a security root.\nCheck oscillator/time flags and require trusted resync.\nProtect time changes with device unlock + confirmation.\nBAT is a protected 1S pack, max 4.2 V.',320,184)
    # RF
    s=sheet('05-nfc','NFC Type 4 tag interface and antenna tuning boundary')
    s.comp('NFC','U11','FM11NC08S (DFN10 + EP)',114.3,91.44,{'1':'+3V3','2':'NFC_VOUT','3':'ANT1','4':'ANT2','5':'GND','6':'NFC_CS_N','7':'NFC_SCK','8':'NFC_MOSI','9':'NFC_MISO','10':'NFC_IRQ_N','11':'GND'},'OpenKeys:FM11NC08S_DFN10_PENDING',note='Fudan official pin map, EP must be checked against orderable package drawing')
    s.comp('CONN2','J5','13.56MHz PCB loop / fixture',287.02,76.2,{'1':'ANT1','2':'ANT2'},'OpenKeys:NFC_LOOP_PENDING',note='Provisional RF window 38x18mm; coil geometry/tuning not released')
    caps(s,[('100nF','+3V3','GND'),('1uF','+3V3','GND'),('100nF 10V','NFC_VOUT','GND'),('DNP C0G tune','ANT1','ANT2')],53.34,172.72)
    s.components[-1]['dnp']=True
    # set schematic DNP too
    for itm in s.items:
        if isinstance(itm,list) and itm and itm[0]=='symbol' and node(itm,'uuid') and node(itm,'uuid')[1]==q(nuid(s.components[-1]['ref'])):node(itm,'dnp')[1]='yes'
    passive(s,'R','47k',226.06,172.72,'+3V3','NFC_IRQ_N');passive(s,'R','47k',264.16,172.72,'+3V3','NFC_CS_N')
    s.text('NFC VOUT is isolated from SYS / BAT / +3V3; no assumed screen or MCU power harvesting.\nFull screen + fingerprint NFC authentication requires a charged battery and device ON.\nDo not route copper/ground/battery/shield under RF window on any PCB layer.\nFM11 has internal resonant capacitance; measure L, Q and resonance in assembled case.\nSchematic does not guarantee ISO14443 / CTAP-over-NFC interoperability.',202,231)
    # UI
    s=sheet('06-trusted-ui','Trusted OLED, device fingerprint boundary, PIN buttons and tamper')
    s.comp('CONN4','J6','OLED 128x64 I2C / SSD1306',66.04,66.04,{'1':'GND','2':'+3V3','3':'UI_SCL','4':'UI_SDA'},'Connector_PinHeader_2.54mm:PinHeader_1x04_P2.54mm_Vertical',note='Pick exact display and confirm pin order, supply and logic voltage')
    s.comp('CONN10','J7','Trusted fingerprint SPI - NOT SELECTED',243.84,68.58,{'1':'FP_3V3','2':'GND','3':'FP_SCK','4':'FP_MOSI','5':'FP_MISO','6':'FP_CS_N','7':'FP_IRQ_N','8':'FP_RESET_N','9':'GND','10':'FP_TAMPER_N'},'OpenKeys:FINGERPRINT_SECURE_MODULE_PENDING',note='Only a contract placeholder; cannot certify UV with unauthenticated match bit')
    passive(s,'FB','600R@100MHz',337.82,66.04,'+3V3','FP_3V3')
    caps(s,[('1uF','+3V3','GND'),('100nF','+3V3','GND'),('10uF','FP_3V3','GND'),('100nF','FP_3V3','GND')],43.18,132.08)
    passive(s,'R','47k',208.28,132.08,'FP_3V3','FP_CS_N');passive(s,'R','47k',246.38,132.08,'FP_3V3','FP_IRQ_N');passive(s,'R','100k',284.48,132.08,'FP_RESET_N','GND')
    passive(s,'R','47k',322.58,132.08,'FP_3V3','FP_TAMPER_N')
    # FP tamper connected to MCU spare PC0, patched in final below.
    for i,(label,net) in enumerate([('UP','BTN_UP_N'),('DOWN','BTN_DOWN_N'),('CONFIRM','BTN_OK_N'),('CANCEL','BTN_CANCEL_N'),('CASE CLOSED','CASE_TAMPER_N')]):
        x=45.72+i*76.2
        s.comp('SW','SW'+str(i+2),label,x,205.74,{'1':net,'2':'GND'},'Button_Switch_SMD:SW_SPST_TL3342' if i<4 else 'OpenKeys:TAMPER_SWITCH_PENDING')
        passive(s,'R','47k',x,226.06,'+3V3',net)
        passive(s,'C','10nF',x+30.48,226.06,net,'GND')
    s.text('Private account labels, credentials, TOTP and recovery notes stay hidden while locked.\nFingerprint must be verified inside a trusted boundary; device PIN fallback has attempt limits.\nOLED is NOT an input device: PIN uses device buttons only.\nCONFIRM/CANCEL apply to one displayed, request-bound action.',181,169)
    # Add FP tamper to PC0 without introducing an unconnected loose wire.
    m=S[2];old=m.components[0];old['nets'][pn['PC0']]='FP_TAMPER_N'
    # Regenerate MCU sheet component while preserving all other items.
    pref='U7';kill={nuid(pref)}
    for n in pmap('MCU'):
        kill.update([nuid(pref+'/'+n+'/nc'),nuid(pref+'/'+n+'/wire'),nuid(pref+'/'+n+'/label')])
    m.items=[it for it in m.items if not(isinstance(it,list) and node(it,'uuid') and str(node(it,'uuid')[1]) in kill)]
    m.components=[c for c in m.components if c['ref']!='U7']
    m.comp('MCU','U7','STM32U585RIT6',160.02,142.24,old['nets'],note=old['note'])
    for idx,(ss,net,x,y) in enumerate([(S[1],'REG_VINA',233.68,228.6),(S[2],'VDDA',370.84,149.86),(S[5],'FP_3V3',370.84,40.64)]):
        ss.comp('PWR','#FLG'+str(idx+5),'PWR_FLAG',x,y,{'1':net})
    for s in S:s.save()
    root=['kicad_sch',['version',20250114],['generator',q('eeschema')],['uuid',q(nuid('OpenKeys'))],['paper',q('A3')],['title_block',['title',q('OpenKeys R0 - Design review')],['date',q('2026-11-08')],['rev',q('0.1')],['company',q('Jay-Void-BA')]],['lib_symbols']]
    root.append(['text',q('OPENKEYS R0 - USB-A + USB-C + NFC authenticator\nEditable circuit design, not fabrication release\nFingerprint module, RF tuning and all physical security tests remain open'),['at',180,30,0],effects(2),['uuid',q(nuid('root/title'))]])
    for i,s in enumerate(S):
        x=30+(i%2)*185;y=60+(i//2)*60
        root.append(['sheet',['at',x,y],['size',155,35],['stroke',['width',0.1524],['type','default']],['fill',['color',0,0,0,0]],['uuid',q(s.uid)],['property',q('Sheetname'),q(s.name),['at',x,y-1,0],effects(1.5)],['property',q('Sheetfile'),q(s.name+'.kicad_sch'),['at',x,y+37,0],effects()],['instances',['project',q('OpenKeys'),['path',q('/'+nuid('OpenKeys')),['page',q(str(i+2))]]]]])
    root.append(['sheet_instances',['path',q('/'),['page',q('1')]]])
    (CAD/'OpenKeys.kicad_sch').write_text(pretty(root)+'\n')
    all_lib=[]
    for k,lib in LIB.items():all_lib.append(lib)
    (CAD/'OpenKeys.kicad_sym').write_text(pretty(['kicad_symbol_lib',['version',20241209],['generator',q('kicad_symbol_editor')],*all_lib])+'\n')
    (CAD/'sym-lib-table').write_text('(sym_lib_table (version 7) (lib (name "OpenKeys") (type "KiCad") (uri "${KIPRJMOD}/OpenKeys.kicad_sym") (options "") (descr "Pinned symbols for OpenKeys R0")))\n')
    (CAD/'OpenKeys.kicad_pro').write_text(json.dumps({'meta':{'filename':'OpenKeys.kicad_pro','version':1},'text_variables':{'DESIGN_STATUS':'NOT_FOR_FABRICATION'}},indent=2)+'\n')
    comps=[c for s in S for c in s.components]
    with (ROOT/'BOM.csv').open('w') as f:
        w=csv.DictWriter(f,fieldnames=['ref','value','footprint','sheet','dnp','note'],lineterminator='\n');w.writeheader();w.writerows({k:c[k] for k in w.fieldnames} for c in comps)
    (ROOT/'review/connections.json').write_text(json.dumps(comps,indent=2)+'\n')
    with (ROOT/'pinout.csv').open('w') as f:
        w=csv.writer(f,lineterminator="\n");w.writerow(['pin','gpio','net','function'])
        af={'PA5':'SPI1_SCK AF5','PA6':'SPI1_MISO AF5','PA7':'SPI1_MOSI AF5','PB13':'SPI2_SCK AF5','PC2':'SPI2_MISO AF5','PB15':'SPI2_MOSI AF5','PB8':'I2C1_SCL AF4','PB9':'I2C1_SDA AF4','PB10':'I2C2_SCL AF4','PB14':'I2C2_SDA AF4','PC10':'SPI3_SCK AF6','PC11':'SPI3_MISO AF6','PC12':'SPI3_MOSI AF6','PA11':'USB_DM AF10','PA12':'USB_DP AF10'}
        for num,net in sorted(old['nets'].items(),key=lambda p:int(p[0])):w.writerow([num,pmap('MCU')[num],net,af.get(pmap('MCU')[num],'power / GPIO / dedicated')])
    print(f'Generated {len(S)+1} sheets and {len(comps)} BOM positions')
if __name__=='__main__':generate()
