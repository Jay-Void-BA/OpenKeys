#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""R0.5 candidate circuits. Output is isolated from the preserved R0.4 design.
No flashing, fabrication, fuse writes or release authorization occurs here.
"""
from schematic import *
LIB['MCU']=library('MCU_ST_STM32H5','STM32H573RITx')
LIB['CHG']=custom('BQ25185',[(1,'SYS','power_out'),(2,'BAT','power_out'),(3,'STAT2','open_collector'),(4,'CE_N','input'),(5,'GND','power_in'),(6,'TS_MR','input'),(7,'ILIM_VSET','passive'),(8,'ISET','passive'),(9,'STAT1','open_collector'),(10,'IN','power_in'),(11,'EP','passive')])
LIB['LCD']=custom('NHD_C0216CiZ',[(1,'RST_N','input'),(2,'SCL','input'),(3,'SDA','bidirectional'),(4,'VSS','power_in'),(5,'VDD','power_in'),(6,'VOUT','passive'),(7,'C1+','passive'),(8,'C1-','passive'),('A','LED_A','passive'),('K','LED_K','passive')])
for k,l,n in [('SUP','Power_Supervisor','TPS3702'),('BUF','74xGxx','74LVC1G07'),('AND','74xGxx','74LVC1G08'),('LDO','Regulator_Linear','TLV70030_SOT23-5')]:LIB[k]=library(l,n)
# TI TPS3702 datasheet pin table explicitly specifies open-drain UV and OV.
# Correct the stock library electrical type; do not suppress ERC conflicts.
for p in pins(LIB['SUP']):
 if str(node(p,'name')[1]) in ('UV','OV'):p[1]='open_collector'
LIB['AON']=library('Regulator_Linear','MCP1700x-300xxTT')
LIB['XTAL']=library('Device','Crystal')
LIB['ZEN']=library('Device','D_Zener')
LIB['EFUSE']=custom('TPS259470L',[(1,'EN_UVLO','input'),(2,'OVLO','input'),(3,'AUXOFF','open_collector'),(4,'FLT_N','open_collector'),(5,'IN','power_in'),(6,'OUT','power_out'),(7,'DVDT','passive'),(8,'GND','power_in'),(9,'ILM','passive'),(10,'ITIMER','passive')])
LIB['LOAD'] = custom('TPS22919_DCK', [
    (1, 'IN', 'power_in'), (2, 'GND', 'power_in'), (3, 'ON', 'input'),
    (4, 'NC', 'no_connect'), (5, 'QOD', 'passive'), (6, 'OUT', 'power_out')])
LIB['LATCH'] = custom('SN74LVC1G74_DCU', [
    (1, 'CLK', 'input'), (2, 'D', 'input'), (3, 'Q_N', 'output'),
    (4, 'GND', 'power_in'), (5, 'Q', 'output'), (6, 'CLR_N', 'input'),
    (7, 'PRE_N', 'input'), (8, 'VCC', 'power_in')])
LIB['SWITCH'] = custom('SN74LVC2G66_DCU', [
    (1, '1A', 'bidirectional'), (2, '1B', 'bidirectional'), (3, '2C', 'input'),
    (4, 'GND', 'power_in'), (5, '2A', 'bidirectional'), (6, '2B', 'bidirectional'),
    (7, '1C', 'input'), (8, 'VCC', 'power_in')])
LIB['SCHMITT']=library('74xGxx','74LVC1G17')
LIB['LEVEL']=custom('TCA9517A_DGK',[(1,'VCCA','power_in'),(2,'SCLA','bidirectional'),(3,'SDAA','bidirectional'),(4,'GND','power_in'),(5,'EN','input'),(6,'SDAB','bidirectional'),(7,'SCLB','bidirectional'),(8,'VCCB','power_in')])
LIB['WINDOW']=custom('TPS3700_DDC',[(1,'OUTA','open_collector'),(2,'GND','power_in'),(3,'INA+','input'),(4,'INB-','input'),(5,'VDD','power_in'),(6,'OUTB','open_collector')])
LIB['RESETDELAY']=custom('TPS3808G33_DBV',[(1,'RESET_N','open_collector'),(2,'GND','power_in'),(3,'MR_N','input'),(4,'CT','input'),(5,'SENSE','input'),(6,'VDD','power_in')])

def named(k,d):return {num:d.get(name) for num,name in pmap(k).items()}
def usb():
 s=sheet('01-usb','One USB-C receptacle - USB 2.0 FS data and 5 V charging')
 s.comp('USBC','J1','HRO TYPE-C-31-M-12',91.44,88.9,{'A1':'GND','A4':'VBUS_RAW','A5':'CC1','A6':'USB_DP','A7':'USB_DM','A8':None,'A9':'VBUS_RAW','A12':'GND','B1':'GND','B4':'VBUS_RAW','B5':'CC2','B6':'USB_DP','B7':'USB_DM','B8':None,'B9':'VBUS_RAW','B12':'GND','S1':'SHIELD'},'Connector_USB:USB_C_Receptacle_HRO_TYPE-C-31-M-12',note='Only external data socket; qualify manufacturer drawing and sample')
 s.comp('ESD','U1','USBLC6-2SC6',218.44,83.82,{'1':'USB_DP','6':'USB_DP','3':'USB_DM','4':'USB_DM','2':'GND','5':'USB_5V'})
 for x,net in [(269.24,'CC1'),(307.34,'CC2')]:passive(s,'R','5.1k 1%',x,88.9,net,'GND')
 passive(s,'R','1M',264.16,154.94,'SHIELD','GND');passive(s,'C','1nF / 2kV',302.26,154.94,'SHIELD','GND',note='KEMET C1206C102KGRACTU;1206;2kV X7R10percent')
 s.comp('NMOS','Q1','2N7002',68.58,210.82,{'1':'USB_5V','2':'GND','3':'VBUS_PRESENT_N'})
 passive(s,'R','100k',109.22,205.74,'+3V3','VBUS_PRESENT_N');passive(s,'R','1M',149.86,205.74,'VBUS_RAW','GND')
 for idx,(net,x) in enumerate([('USB_5V',210.82),('CC1',264.16),('CC2',317.5)]):
  s.comp('TVS','D'+str(idx+1),'SMF5.0A' if idx==0 else 'PESD5V0S1BA',x,220.98,{'1':net,'2':'GND'},'Diode_SMD:D_SOD-123F' if idx==0 else 'Diode_SMD:D_SOD-323')
 for i,net in enumerate(['VBUS_RAW','GND']):s.comp('PWR','#FLG'+str(i+1),'PWR_FLAG',254+i*45.72,40.64,{'1':net})
 s.text('5.1k Rd on each CC pin: sink only. No PD negotiation, no 9 V request.\nApple 20 W adapter uses 5 V through a standards-compliant USB-C cable.\n100mA hardware input limit: 5 V x 0.1 A = 0.50 W nominal input budget.\nInput guard U14 is independent of MCU: see sheet08. Analog timing/thermal protection not yet qualified.',205,270)
def power():
 s=sheet('02-power','Low-rate charging, protected 500 mAh pack and 3.3 V supply')
 passive(s,'FUSE','250mA hold - qualify I2t',33.02,68.58,'VBUS_RAW','VBUS_FUSED')
 s.comp('CHG','U2','BQ25185DLHR',139.7,73.66,named('CHG',{'IN':'USB_5V','SYS':'SYS','BAT':'BAT','GND':'GND','EP':'GND','CE_N':'CHARGE_DISABLE','TS_MR':'BAT_NTC','ILIM_VSET':'CHG_ILIM_VSET','ISET':'CHG_ISET','STAT1':'CHG_STAT1_N','STAT2':'CHG_STAT2_N'}),'Package_DFN_QFN:Texas_DLH0010A_WSON-10-1EP_2.2x2mm_P0.4mm_EP0.9x1.5mm',note='TI SLUSF65B table6-1:24k=4.2V/input100mA; ISET3k=target100mA before input DPM')
 s.comp('CONN2','J2','Adafruit 1578 protected 1S 500mAh',322.58,76.2,{'1':'BAT','2':'GND'},'Connector_JST:JST_PH_S2B-PH-K_1x02_P2.00mm_Horizontal',note='Factory-protected pack; JST-PH2 polarity must be measured before mating. Pack body29x36x4.75mm; reserve32x40x6.5mm. No bare-cell substitution')
 s.comp('CONN2','J6','103AT-2 external battery thermistor',360.68,137.16,{'1':'BAT_NTC','2':'GND'},'Connector_JST:JST_SH_SM02B-SRSS-TB_1x02-1MP_P1.00mm_Horizontal',note='Semitec103AT-2 10k/B3435 insulated thermal contact to pouch, factory crimped SH harness. Not an ambient-only board thermistor')
 passive(s,'R','24k 1%',66.04,137.16,'CHG_ILIM_VSET','GND');passive(s,'R','3.00k 1%',109.22,137.16,'CHG_ISET','GND')
 passive(s,'R','100k',157.48,137.16,'SYS','CHARGE_DISABLE')
 s.comp('NMOS','Q2','2N7002',264.16,137.16,{'1':'CHARGE_ENABLE','2':'GND','3':'CHARGE_DISABLE'})
 passive(s,'R','100k',307.34,137.16,'CHARGE_ENABLE','GND')
 caps(s,[('1uF 10V','USB_5V','GND'),('10uF 6.3V','BAT','GND'),('10uF 10V','SYS','GND')],33.02,187.96)
 s.comp('PWR','#FLG3','PWR_FLAG',73.66,35.56,{'1':'VBUS_FUSED'})
 s.comp('SPDT','SW1','OFF / ON',190.5,220.98,{'1':'SYS','2':'REG_EN','3':'GND'},'Button_Switch_SMD:SW_SPDT_CK_JS102011SAQN')
 s.comp('REG','U3','TPS63031DSK (3.3 V)',289.56,223.52,named('REG',{'VIN':'SYS','VINA':'REG_VINA','VOUT':'+3V3','FB':'+3V3','EN':'REG_EN','PS/SYNC':'SYS','L1':'SW_L1','L2':'SW_L2','GND':'GND','PGND':'GND','EP':'GND'}),note='PS/SYNC tied to VIN: forced PWM, guaranteed DC output3.267..3.333V before load/transient effects. GND power-save mode has wider -3/+6percent range and is not approved here.')
 passive(s,'R','0R',210.82,177.8,'SYS','REG_VINA');passive(s,'L','1.5uH Isat>=1.5A',353.06,152.4,'SW_L1','SW_L2')
 caps(s,[('10uF 10V','SYS','GND'),('100nF','REG_VINA','GND'),('10uF 6.3V','+3V3','GND'),('10uF 6.3V','+3V3','GND')],35.56,241.3)
 s.comp('PWR','#FLG4','PWR_FLAG',391.16,198.12,{'1':'REG_VINA'})
 s.text('Fixed100mA input; charge target100mA=0.2C for500mAh.\nInput DPM gives system load priority, so cell charge can be slower.\nCE defaults disabled. Public charging service never unlocks secrets.\nSuspend: stop charge/backlight; qualify total VBUS current <=2.5mA.\nReal pack PCM + attached thermistor required. No fast charge.',111,279,1.15)
def mcu():
 s=sheet('03-mcu','STM32H573RIT6 - STiRoT and TrustZone protected I/O')
 pm={num:name.split('(')[0] for num,name in pmap('MCU').items()};pn={v:k for k,v in pm.items()};nets={}
 for num,name in pm.items():
  if name=='VDD':nets[num]='+3V3'
  elif name=='VBAT':nets[num]='TAMP_3V0'
  elif name=='VDDA':nets[num]='VDDA'
  elif name in ('VSS','VSSA'):nets[num]='GND'
 signals={'NRST':'NRST','BOOT0':'BOOT0','PA2':'CHG_STAT1_N','PA3':'CHG_STAT2_N','PA4':'NFC_CS_N','PA5':'NFC_SCK','PA6':'NFC_MISO','PA7':'NFC_MOSI','PA8':'LCD_HOST_SCL','PA11':'USB_DM','PA12':'USB_DP','PA13':'SWDIO','PA14':'SWCLK','PA15':'BTN7_N','PB0':'BTN1_N','PB1':'BTN2_N','PB2':'BTN3_N','PB3':'SWO','PB4':'CHARGE_ENABLE','PB5':'BTN4_N','PB6':'SE_SCL','PB7':'SE_SDA','PB10':'NFC_IRQ_N','PB12':'FLASH_CS_N','PB13':'FLASH_SCK','PB15':'FLASH_MOSI','PC0':'USB_GUARD_FAULT_N','PC1':'NRST','PC2':'FLASH_MISO','PC3':'LCD_RELEASE','PC4':'LCD_ARM_CLK','PC5':'VBUS_PRESENT_N','PC6':'BTN5_N','PC7':'BTN6_N','PC8':'SE_ENABLE_REQUEST','PC9':'LCD_HOST_SDA','PC10':'BTN8_N','PC11':'BTN9_N','PC12':'BTN0_N','PC13':'CASE_CLOSED','PC14':'LSE_IN','PC15':'LSE_OUT'}
 for pin,net in signals.items():nets[pn[pin]]=net
 nets['30']='VCAP1';nets['62']='VCAP2'
 s.comp('MCU','U4','STM32H573RIT6',160.02,142.24,nets,note='H573 pin62=VCAP2, no PB9; SE PB6/PB7; LCD PA8/PC9. Recheck complete alternate functions before firmware target release')
 caps(s,[('100nF','+3V3','GND')]*5+[('4.7uF','+3V3','GND'),('2.2uF low ESR','VCAP1','GND'),('2.2uF low ESR','VCAP2','GND'),('100nF','VDDA','GND'),('1uF','VDDA','GND')],30.48,35.56)
 passive(s,'FB','600R@100MHz',302.26,88.9,'+3V3','VDDA');passive(s,'R','10k',309.88,124.46,'+3V3','NRST');passive(s,'C','10nF - qualify reset timing',347.98,124.46,'NRST','GND');passive(s,'R','10k',309.88,157.48,'BOOT0','GND')
 s.comp('CONN10','J3','Internal SWD - DNP production',309.88,218.44,{'1':'+3V3','2':'SWDIO','3':'GND','4':'SWCLK','5':'GND','6':'SWO','7':None,'8':None,'9':'GND','10':'NRST'},'Connector_PinHeader_1.27mm:PinHeader_2x05_P1.27mm_Vertical',dnp=True,note='Internal manufacturing access only; close debug lifecycle before user secrets')
 s.comp('XTAL','Y1','ABS07-32.768KHZ-6-T',317.5,190.5,{'1':'LSE_IN','2':'LSE_OUT'},'Crystal:Crystal_SMD_3215-2Pin_3.2x1.5mm',note='LSE for backup RTC and tamper. RTC validity/rollback must be qualified. 9pF loading provisional; qualify startup/drive across temperature')
 passive(s,'C','9pF C0G - tune',363.22,185.42,'LSE_IN','GND');passive(s,'C','9pF C0G - tune',398.78,185.42,'LSE_OUT','GND')
 s.comp('PWR','#FLG6','PWR_FLAG',381,91.44,{'1':'VDDA'})
 s.text('STiRoT must verify protected firmware. TrustZone/GTZC must deny\nnonsecure access to SE, LCD, ten keys, timers, DMA and key memory.\nNo raw cryptography or arbitrary display commands at secure gateway.\nSE_EN is a fault interlock, not proof of a correct PIN.\nSeparate2.2uF on VCAP1/2; no external load on VCAP.',129,258)
 af={'PA5':'SPI1_SCK AF5','PA6':'SPI1_MISO AF5','PA7':'SPI1_MOSI AF5','PB13':'SPI2_SCK AF5','PC2':'SPI2_MISO AF5','PB15':'SPI2_MOSI AF5','PB6':'I2C1_SCL AF4','PB7':'I2C1_SDA AF4','PA8':'I2C3_SCL AF4','PC9':'I2C3_SDA AF4','PA11':'USB_DM AF10','PA12':'USB_DP AF10','PC1':'TAMP_IN3 dedicated','PC13':'TAMP_IN1 dedicated/VBAT'}
 with (ROOT/'pinout.csv').open('w') as f:
  w=csv.writer(f,lineterminator='\n');w.writerow(['pin','gpio','net','function'])
  for num,net in sorted(nets.items(),key=lambda p:int(p[0])):w.writerow([num,pm[num],net,af.get(pm[num],'power / GPIO / dedicated')])
def storage():
 s=sheet('04-se-storage','Secure element, protected keys and encrypted records')
 s.comp('SE','U5','SE051C2HQ1/Z01XDZ',78.74,83.82,{'1':'GND','9':'SE_SDA','10':'SE_SCL','11':'SE_EN','12':'+3V3','14':'GND','15':'SE_CORE','17':'GND','18':'SE_CORE','19':'GND','21':'GND'},'OpenKeys:SE050_SOT1969-1_HX2QFN20',note='SE051 Rev2.0 table3/SOT1969-1: unused RF1/17 and RST14 grounded; VOUT15 to VCC18 only. Target C2 OEF A8FA/Applet7.2 full feature. Identity, PIN ACL/retries, RSA4096 and per-device SCP commissioning still require qualification')
 s.comp('FLASH','U6','W25Q64JVSSIQ',236.22,83.82,named('FLASH',{'VCC':'+3V3','GND':'GND','~{CS}':'FLASH_CS_N','CLK':'FLASH_SCK','DI/IO_{0}':'FLASH_MOSI','DO/IO_{1}':'FLASH_MISO','~{WP}/IO_{2}':'FLASH_WP_N','~{HOLD}/~{RESET}/IO_{3}':'FLASH_HOLD_N'}))
 for i,(a,b,v) in enumerate([('+3V3','SE_SCL','4.7k'),('+3V3','SE_SDA','4.7k'),('SE_EN','GND','100k'),('+3V3','FLASH_CS_N','47k'),('+3V3','FLASH_WP_N','47k'),('+3V3','FLASH_HOLD_N','47k'),('+3V3','CHG_STAT1_N','10k'),('+3V3','CHG_STAT2_N','10k')]):passive(s,'R',v,30.48+i*48.26,154.94,a,b)
 caps(s,[('100nF','+3V3','GND'),('100nF','SE_CORE','GND'),('1uF','SE_CORE','GND'),('100nF','+3V3','GND')],50.8,215.9)
 s.text('Private operations: valid local six-digit PIN grant + per-request confirmation.\nNo biometric path. SE-native PIN ACLs and durable retries must be qualified; Root owns keypad.\nPrecharge attempts durably; uncertain writes deny. AEAD does not alone prevent rollback.\nSE certification does not certify MCU, PCB, PIN execution path or complete product.',184,264)
def nfc():
 s=sheet('05-nfc','NFC Type4 card interface - battery-powered local PIN UI')
 s.comp('NFC','U7','FM11NC08S (DFN10 + EP)',114.3,91.44,{'1':'+3V3','2':'NFC_VOUT','3':'ANT1','4':'ANT2','5':'GND','6':'NFC_CS_N','7':'NFC_SCK','8':'NFC_MOSI','9':'NFC_MISO','10':'NFC_IRQ_N','11':'GND'},'OpenKeys:FM11NC08S_DFN10_3x3_EP1.7x2.4',note='Original candidate lands from Fudan v1.0 p9: package3x3,pitch0.5,EP rotated1.7x2.4; confirm supplier revision, sample orientation, paste and DFM')
 s.comp('CONN2','J4','Integral 13.56MHz PCB loop',287.02,76.2,{'1':'ANT1','2':'ANT2'},'OpenKeys:NFC_PCB_Loop_4T_62x15',note='Integral copper4-turn coil; no purchased connector. Separate RF-only region. Initial matching model must be measured in assembled shell.')
 caps(s,[('100nF','+3V3','GND'),('1uF','+3V3','GND'),('100nF 10V','NFC_VOUT','GND')],53.34,172.72)
 passive(s,'C','56pF C0G 50V initial match',154.94,172.72,'ANT1','ANT2')
 passive(s,'R','47k',226.06,172.72,'+3V3','NFC_IRQ_N');passive(s,'R','47k',264.16,172.72,'+3V3','NFC_CS_N')
 s.text('NFC VOUT isolated from SYS/BAT/+3V3. No assumed RF-powered full device.\nLCD/PIN/CTAP NFC require charged battery and device ON.\nRF region: tuned coil only, no cell, metal shield or ground planes.\nMobile management needs authenticated APDUs and native OS integration.\nPhone biometrics or a tap never replace device PIN verification.',190,241)
def trusted_ui():
 s=sheet('06-trusted-ui','DFR0555 blue 16x2 LCD and ten independent PIN keys')
 s.comp('CONN4','J7','DFR0555 V1.1 - OFFBOARD MODULE',88.9,71.12,{'1':'LCD_VCC_SW','2':'GND','3':'LCD_SCL','4':'LCD_SDA'},'Connector_PinHeader_2.54mm:PinHeader_1x04_P2.54mm_Vertical',note='Internal keyed harness must follow VCC/GND/SCL/SDA; no external second data port. Module87x32x13mm. Qualify actual supply tolerance and revision.')
 for i,net in enumerate(['LCD_HOST_SCL','LCD_HOST_SDA']):passive(s,'R','4.7k',43.18+i*53.34,132.08,'+3V3',net)
 for i,digit in enumerate([1,2,3,4,5,6,7,8,9,0]):
  xx=35.56+(i%5)*73.66; yy=172.72+(i//5)*66.04; net='BTN'+str(digit)+'_N'
  s.comp('SW','SW'+str(i+2),'KEY '+str(digit),xx,yy,{'1':net,'2':'GND'},'Button_Switch_SMD:SW_SPST_TL3342',note='Independent GPIO; physical release events only. No matrix and no host key injection.')
  passive(s,'R','47k',xx,yy+20.32,'+3V3',net);passive(s,'C','10nF',xx+25.4,yy+20.32,net,'GND')
 s.text('ENGLISH 16x2, blue background / white text. No fingerprint parts.\nPIN:6 decimal digits, masked. Hold9=erase; hold0=cancel.\nReview:2=previous,8=next,5=confirm,0=cancel. New release required.\nPrivate operations require a valid paired30s local PIN grant and exact review.\nNo PIN from USB/NFC/desktop. No factory/default unlock PIN.',195,283,1.1)
def fault_tamper():
 s=sheet('07-fault-tamper','Independent reset, battery-backed case tamper and SE interlock')
 s.comp('AON','U13','MCP1700T-3002E/TT',337.82,40.64,named('AON',{'VI':'SYS','VO':'TAMP_3V0','GND':'GND'}),note='Always-on3.0V from charger SYS: USB or battery, independent of main switch. SYS must stay within MCP1700 6V maximum. Qualify reverse injection and source transitions')
 caps(s,[('4.7uF 10V X7R','SYS','GND'),('4.7uF 10V X7R','TAMP_3V0','GND')],335.28,96.52)
 s.comp('C','C56','100nF X7R 16V',393.7,40.64,{'1':'TAMP_3V0','2':'GND'},'Capacitor_SMD:C_0603_1608Metric',note='Local U4.1 VBAT decoupling, separate from U13 output capacitor. Both U13 capacitors must retain effective capacitance >=1uF across bias/temperature/tolerance')
 s.comp('SUP','U10','TPS3702CX33DDCR',99.06,78.74,named('SUP',{'SENSE':'+3V3','SET':'+3V3','VDD':'SYS','GND':'GND','UV':'NRST','OV':'NRST'}),'Package_TO_SOT_SMD:TSOT-23-6',note='SET high: nominal UV3.17V/OV3.43V; open-drain reset independent of ADC/software. Not nanosecond fault protection')
 s.comp('SW','SW12','LID CLOSED contact',246.38,68.58,{'1':'TAMP_3V0','2':'CASE_CLOSED'},'Button_Switch_SMD:SW_SPST_TL3342',note='Lid actuator closes contact; open wire pulls low => independent reset. Travel/contact reliability unvalidated')
 passive(s,'R','100k',292.1,68.58,'CASE_CLOSED','GND')
 s.comp('BUF','U11','SN74LVC1G07DCKR',246.38,124.46,{'1':None,'2':'CASE_CLOSED','3':'GND','4':'NRST','5':'+3V3'},'Package_TO_SOT_SMD:SOT-353_SC-70-5',note='Open-drain case-open reset; cannot drive reset high against supervisor')
 s.comp('AND','U12','SN74LVC1G08DCKR',99.06,172.72,{'1':'NRST','2':'SE_ENABLE_REQUEST','3':'GND','4':'SE_EN','5':'+3V3'},'Package_TO_SOT_SMD:SOT-353_SC-70-5',note='Fault/reset disables SE_EN independently of request. Fault interlock only; not a PIN authorization gate')
 passive(s,'R','100k',154.94,172.72,'SE_ENABLE_REQUEST','GND')
 caps(s,[('100nF 10V','SYS','GND'),('100nF','+3V3','GND'),('100nF','+3V3','GND')],246.38,177.8)
 s.text('Reset invalidates volatile grants; reboot must verify firmware and require fresh device PIN.\nPC13=case input; PC1/TAMP_IN3 observes reset/fault line.\nInternal BOR/PVD, clock checks and persistent precharged attempts also required.\nProduction debug lifecycle closed; no populated external debug connector.\nVBAT+LSE must retain tamper with main OFF; full backup loss must lock, not initialize.\nMeasure reset/ENA latency, leakage and injection before security release.',187,255)
def input_guard():
 s=sheet('08-input-guard','Independent VBUS overvoltage cutoff, reverse blocking and limited inrush')
 s.comp('EFUSE','U14','TPS259470LRPWR',144.78,86.36,named('EFUSE',{'EN_UVLO':'USB_GUARD_EN','OVLO':'USB_GUARD_OV','AUXOFF':None,'FLT_N':'USB_GUARD_FAULT_N','IN':'VBUS_FUSED','OUT':'USB_5V','DVDT':'USB_GUARD_DVDT','GND':'GND','ILM':'USB_GUARD_ILM','ITIMER':None}),'OpenKeys:TPS25947_RPW0010A',note='Adjustable OVLO, latch-off variant for latched faults, true reverse-current blocking. Follow datasheet fault-specific recovery; do not infer all OV events latch. Input abs28V is not a system surge rating.')
 for i,(v,a,b) in enumerate([('22.1k 1%','VBUS_FUSED','USB_GUARD_EN'),('10k 1%','USB_GUARD_EN','GND'),('374k 0.1%','VBUS_FUSED','USB_GUARD_OV'),('100k 0.1%','USB_GUARD_OV','GND'),('6.49k 1%','USB_GUARD_ILM','GND'),('47k','+3V3','USB_GUARD_FAULT_N')]):passive(s,'R',v,35.56+i*63.5,157.48,a,b)
 s.comp('ZEN','D4','BZT52-C5V1,115',58.42,88.9,{'1':'USB_GUARD_EN','2':'GND'},'Diode_SMD:D_SOD-123',note='Protect EN/UVLO absolute maximum6.5V; not tied directly to raw VBUS. Verify leakage/temp and dynamic clamp.')
 s.comp('TVS','D5','SMF16A',302.26,86.36,{'1':'VBUS_FUSED','2':'GND'},'Diode_SMD:D_SOD-123F',note='Input surge TVS AFTER F1. Nominal datasheet clamp26V vs eFuse28Vabs is not an IEC guarantee; qualify actual diode supplier, layout and I2t.')
 caps(s,[('100nF 50V','VBUS_FUSED','GND'),('1nF C0G','USB_GUARD_DVDT','GND')],246.38,223.52)
 s.text('OVLO nominal1.2V x(1+374k/100k)=5.688V. Component/leakage corners in protection-calculations.json.\nUVLO divider22.1k/10k: about3.85V nominal. EN is separately clamped, never raw VBUS.\n6.49k ILM: about0.51A protection threshold; BQ charger still sets100mA input budget.\n1nF DVDT: about2V/ms, about2.5ms ramp to5V; measure actual inrush with complete load.\nF1 -> input TVS -> eFuse -> output TVS/ESD reference/charger/sense.\nIndependent cutoff is a circuit provision; response overshoot, CC/data faults and thermal/surge tests remain BLOCKED.',195,263)
def lcd_guard():
 s=sheet('09-display-power','Latched LCD power removal; isolated bus; no resetless retained text')
 s.comp('BUF','U15','SN74LVC1G07DCKR',35.56,63.5,{'1':None,'2':'NRST','3':'GND','4':'LCD_RELEASE','5':'+3V3'},'Package_TO_SOT_SMD:SOT-353_SC-70-5',note='NRST low asynchronously clears display power latch via U17. PC3 is open-drain; no repower without a new PC4 edge.')
 passive(s,'R','10k',35.56,119.38,'+3V3','LCD_RELEASE')
 passive(s,'R','100k',35.56,152.4,'LCD_RELEASE','GND')
 passive(s,'R','100k',35.56,185.42,'LCD_ARM_CLK','GND')
 passive(s,'C','100nF',35.56,215.9,'+3V3','GND')
 # TI pin tables are literal and independently checked by check_netlist.py.
 items=[('RESETDELAY', 'U3', 'TPS3808G33DBVR', 106.68, 63.5, {'1': 'CLEAR_N', '2': 'GND', '3': 'LCD_RELEASE', '4': 'LCD_RESET_CT', '5': '+3V3', '6': '+3V3'}, 'Package_TO_SOT_SMD:SOT-23-6'), ('LATCH', 'U2', 'SN74LVC1G74DCUR', 185.42, 66.04, {'1': 'LCD_ARM_CLK', '2': '+3V3', '3': None, '4': 'GND', '5': 'POWER_LATCH', '6': 'CLEAR_N', '7': '+3V3', '8': '+3V3'}, 'Package_SO:VSSOP-8_2.3x2mm_P0.5mm'), ('LOAD', 'U1', 'TPS22919DCKR', 266.7, 66.04, {'1': 'SYS', '2': 'GND', '3': 'POWER_LATCH', '4': None, '5': 'LCD_VCC_SW', '6': 'LCD_VCC_SW'}, 'Package_TO_SOT_SMD:SOT-363_SC-70-6'), ('SCHMITT', 'U4', 'SN74LVC1G17DCKR', 106.68, 129.54, {'1': None, '2': 'CONNECT_DELAY', '3': 'GND', '4': 'DELAY_READY', '5': '+3V3'}, 'Package_TO_SOT_SMD:SOT-353_SC-70-5'), ('AND', 'U5', 'SN74LVC1G08DCKR', 185.42, 129.54, {'1': 'POWER_LATCH', '2': 'DELAY_READY', '3': 'GND', '4': 'BUS_ENABLE', '5': '+3V3'}, 'Package_TO_SOT_SMD:SOT-353_SC-70-5'), ('LEVEL', 'U6', 'TCA9517ADGKR', 274.32, 129.54, {'1': '+3V3', '2': 'HOST_SCL', '3': 'HOST_SDA', '4': 'GND', '5': 'LCD_BUFFER_EN', '6': 'LCD_SDA', '7': 'LCD_SCL', '8': 'LCD_VCC_SW'}, 'Package_SO:VSSOP-8_3x3mm_P0.65mm')]
 names={'U1':'U18','U2':'U16','U3':'U17','U4':'U19','U5':'U20','U6':'U21'}
 for kind,ref,value,x,y,nets,footprint in items:
  if ref not in names:continue
  nets={k:({'HOST_SCL':'LCD_HOST_SCL','HOST_SDA':'LCD_HOST_SDA'}.get(v,v)) for k,v in nets.items()}
  s.comp(kind,names[ref],value,x+15.24,y,nets,footprint,note='Independent reset hold: CT through100k to VDD gives180..420ms; MR=NRST/PC3 sum. Early PC4 pulses cannot rearm Q while RESET_N low.' if names[ref]=='U17' else '')
 passive(s,'R','220k 1%',104.14,185.42,'POWER_LATCH','CONNECT_DELAY')
 passive(s,'R','330R 1% 0.25W',160.02,185.42,'LCD_VCC_SW','GND',note='330ohm bleeder remains effective without input power. Specify rated0.25W1206; qualify total module+board C<=100uF and off injection<=20uA. Extra display-on dissipation up to about73mW must fit input budget.')
 passive(s,'R','100k',215.9,185.42,'BUS_ENABLE','GND')
 caps(s,[('100nF 10% X7R 16V','CONNECT_DELAY','GND'),('1uF 10V','SYS','GND'),('1uF 10V','LCD_VCC_SW','GND')]+[('100nF','+3V3','GND')]*5,50.8,241.3)
 s.text('U16 D=1, PRE_N=1. NRST/PC3 low clears Q through U17.\nCT through100k to VDD: RESET_N stays low180..420ms after MR release.\nReset release alone does not restore LCD power; a NEW PC4 edge is required.\nQ controls TPS22919; Q AND delayed-Q drives U21 EN through U23 on sheet10.\nLock: PC4 low, PC3 low, wait500ms. Restart: PC3 high, wait500ms,\nPC4 high, wait200ms, then full LCD initialization. Any I/O fault -> OFF.\nLCD rail is switched SYS; A/B level buffering isolates the3.3V MCU bus.\nPhysical clearing, injection, timing and power qualification NOT_RUN.',210,97,1.3)
def supply_window():
 s=sheet('10-sys-window','SYS voltage window and LCD level-buffer fail-closed enable')
 s.comp('WINDOW','U22','TPS3700DDCR',73.66,76.2,{'1':'NRST','2':'GND','3':'SYS_UV_SENSE','4':'SYS_OV_SENSE','5':'+3V3','6':'NRST'},'Package_TO_SOT_SMD:TSOT-23-6',note='Cross-powered by main3.3V; senses SYS independently of software. OUTA/OUTB open drain join NRST. U10 is powered from SYS and watches main3.3V. Typical propagation is not nanosecond FI protection.')
 for value,x,y,a,b in [('78.7k 0.1%',25.4,154.94,'SYS','SYS_UV_SENSE'),('10k 0.1%',81.28,154.94,'SYS_UV_SENSE','GND'),('110k 0.1%',137.16,154.94,'SYS','SYS_OV_SENSE'),('10k 0.1%',193.04,154.94,'SYS_OV_SENSE','GND')]:passive(s,'R',value,x,y,a,b)
 s.comp('BUF','U23','SN74LVC1G07DCKR',299.72,76.2,{'1':None,'2':'BUS_ENABLE','3':'GND','4':'LCD_BUFFER_EN','5':'+3V3'},'Package_TO_SOT_SMD:SOT-353_SC-70-5',note='Non-inverting open-drain level conversion; TCA9517A EN high threshold tracks VCCB and cannot be driven directly by3.3V at highest SYS. Output5.5V tolerant with Ioff. Low BUS_ENABLE forces buffer off.')
 for value,x,y,a,b in [('10k 1%',248.92,154.94,'LCD_VCC_SW','LCD_BUFFER_EN'),('100k 1%',304.8,154.94,'LCD_BUFFER_EN','GND'),('4.7k',248.92,205.74,'LCD_VCC_SW','LCD_SCL'),('4.7k',304.8,205.74,'LCD_VCC_SW','LCD_SDA')]:passive(s,'R',value,x,y,a,b)
 caps(s,[('100nF','LCD_VCC_SW','GND'),('100nF','+3V3','GND'),('100nF','+3V3','GND')],43.18,236.22)
 passive(s,'R','10k 1%',27.94,198.12,'+3V3','CLEAR_N',note='TPS3808 open-drain reset pullup; not connected to NRST, avoids feedback')
 passive(s,'R','100k 1%',83.82,198.12,'+3V3','LCD_RESET_CT',note='CT requires40k..200k to VDD for fixed300ms mode. Do not short CT directly to VDD or fit a timing capacitor.')
 passive(s,'R','10k 1%',139.7,198.12,'POWER_LATCH','GND',note='Forces TPS22919 ON low when latch/main supply is unpowered; defeats back-charge from CONNECT_DELAY through220k')
 caps(s,[('100nF X7R 16V 10%','+3V3','GND')],182.88,236.22)
 s.text('TPS3700 reference: positive396..404mV; negative387..400mV.\nInclude0.1percent divider errors and +/-25nA input bias.\nStatic SYS UV fall min3.4246V; OV rise max4.8596V.\nWith ASSUMED switch+harness drop<=50mV: LCD>=3.3746V.\nMain3.3V fixed PWM:3.267..3.333V; no ripple/transient guarantee.\nTCA sideA=main3.3V; sideB=switched SYS. Off I2C must stay high-Z.\nEN high follows switched VCCB through U23, R53/R54.\nLCD total C<=100uF and aggregate off injection<=20uA MUST be measured.\nHardware hold180ms minus ASSUMED turn-off<=5ms permits discharge.\nServices stay locked during startup, undervoltage and all faults.\nFast pulses, backfeed, input budget, temperature and RF remain unqualified.',195,112,1.3)
def save():
 for ss in S:ss.save()
 root=['kicad_sch',['version',20250114],['generator',q('eeschema')],['uuid',q(nuid('OpenKeys'))],['paper',q('A3')],['title_block',['title',q('OpenKeys R0.5 - Engineering review')],['date',q('2026-11-13')],['rev',q('0.5')],['company',q('Jay-Void-BA')]],['lib_symbols']]
 root.append(['text',q('OPENKEYS R0.5 - ONE USB-C + NFC / BLUE 16x2 LCD\nDevice PIN mandatory / TEN DECIMAL KEYS / NO FINGERPRINT\nPCB candidate; qualification required before secret provisioning'),['at',190,28,0],effects(2),['uuid',q(nuid('root/title'))]])
 for i,ss in enumerate(S):
  x=20+(i%3)*128;y=65+(i//3)*51
  root.append(['sheet',['at',x,y],['size',115,31],['stroke',['width',0.1524],['type','default']],['fill',['color',0,0,0,0]],['uuid',q(ss.uid)],['property',q('Sheetname'),q(ss.name),['at',x,y-1,0],effects(1.5)],['property',q('Sheetfile'),q(ss.name+'.kicad_sch'),['at',x,y+33,0],effects()],['instances',['project',q('OpenKeys'),['path',q('/'+nuid('OpenKeys')),['page',q(str(i+2))]]]]])
 root.append(['sheet_instances',['path',q('/'),['page',q('1')]]])
 (CAD/'OpenKeys.kicad_sch').write_text(pretty(root)+'\n')
 used={key:lib for ss in S for key,lib in ss.used.items()}
 (CAD/'OpenKeys.kicad_sym').write_text(pretty(['kicad_symbol_lib',['version',20241209],['generator',q('kicad_symbol_editor')],*used.values()])+'\n')
 (CAD/'sym-lib-table').write_text('(sym_lib_table (version 7) (lib (name "OpenKeys") (type "KiCad") (uri "${KIPRJMOD}/OpenKeys.kicad_sym") (options "") (descr "R0.5 engineering symbols")))\n')
 project_path=CAD/'OpenKeys.kicad_pro'
 project=json.loads(project_path.read_text()) if project_path.exists() else {'meta':{'filename':'OpenKeys.kicad_pro','version':1}}
 project.setdefault('text_variables',{})['DESIGN_STATUS']='BOARD_CANDIDATE_NOT_QUALIFIED'
 project_path.write_text(json.dumps(project,indent=2)+'\n')
 comps=[c for ss in S for c in ss.components]
 assert len({c['ref'] for c in comps})==len(comps),'Duplicate component reference'
 with (ROOT/'BOM.csv').open('w') as f:
  w=csv.DictWriter(f,fieldnames=['ref','value','footprint','sheet','dnp','note'],lineterminator='\n');w.writeheader();w.writerows({k:c[k] for k in w.fieldnames} for c in comps)
 (ROOT/'review/connections.json').write_text(json.dumps(comps,indent=2)+'\n')
 print(f'R0.5: {len(S)+1} sheets, {len(comps)} BOM positions')
if __name__=='__main__':
 usb();power();mcu();storage();nfc();trusted_ui();fault_tamper();input_guard();lcd_guard();supply_window();save()
