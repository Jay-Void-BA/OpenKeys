#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""R0.4 candidate circuits. Output is isolated from the preserved R0.2 design.
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
 s.comp('REG','U3','TPS63031DSK (3.3 V)',289.56,223.52,named('REG',{'VIN':'SYS','VINA':'REG_VINA','VOUT':'+3V3','FB':'+3V3','EN':'REG_EN','PS/SYNC':'GND','L1':'SW_L1','L2':'SW_L2','GND':'GND','PGND':'GND','EP':'GND'}))
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
 signals={'NRST':'NRST','BOOT0':'BOOT0','PA2':'CHG_STAT1_N','PA3':'CHG_STAT2_N','PA4':'NFC_CS_N','PA5':'NFC_SCK','PA6':'NFC_MISO','PA7':'NFC_MOSI','PA8':'LCD_SCL','PA11':'USB_DM','PA12':'USB_DP','PA13':'SWDIO','PA14':'SWCLK','PA15':'FP_CS_N','PB0':'BTN1_N','PB1':'BTN2_N','PB2':'BTN3_N','PB3':'SWO','PB4':'CHARGE_ENABLE','PB5':'BTN4_N','PB6':'SE_SCL','PB7':'SE_SDA','PB10':'NFC_IRQ_N','PB12':'FLASH_CS_N','PB13':'FLASH_SCK','PB15':'FLASH_MOSI','PC0':'USB_GUARD_FAULT_N','PC1':'NRST','PC2':'FLASH_MISO','PC3':'LCD_RESET_N','PC4':'LCD_BACKLIGHT','PC5':'VBUS_PRESENT_N','PC6':'FP_IRQ','PC7':'FP_RESET_N','PC8':'SE_ENABLE_REQUEST','PC9':'LCD_SDA','PC10':'FP_SCK','PC11':'FP_MISO','PC12':'FP_MOSI','PC13':'CASE_CLOSED','PC14':'LSE_IN','PC15':'LSE_OUT'}
 for pin,net in signals.items():nets[pn[pin]]=net
 nets['30']='VCAP1';nets['62']='VCAP2'
 s.comp('MCU','U4','STM32H573RIT6',160.02,142.24,nets,note='H573 pin62=VCAP2, no PB9; SE PB6/PB7; LCD PA8/PC9. Recheck complete alternate functions before firmware target release')
 caps(s,[('100nF','+3V3','GND')]*5+[('4.7uF','+3V3','GND'),('2.2uF low ESR','VCAP1','GND'),('2.2uF low ESR','VCAP2','GND'),('100nF','VDDA','GND'),('1uF','VDDA','GND')],30.48,35.56)
 passive(s,'FB','600R@100MHz',302.26,88.9,'+3V3','VDDA');passive(s,'R','10k',309.88,124.46,'+3V3','NRST');passive(s,'C','10nF - qualify reset timing',347.98,124.46,'NRST','GND');passive(s,'R','10k',309.88,157.48,'BOOT0','GND')
 s.comp('CONN10','J3','Internal SWD - DNP production',309.88,218.44,{'1':'+3V3','2':'SWDIO','3':'GND','4':'SWCLK','5':'GND','6':'SWO','7':None,'8':None,'9':'GND','10':'NRST'},'Connector_PinHeader_1.27mm:PinHeader_2x05_P1.27mm_Vertical',dnp=True,note='Internal manufacturing access only; close debug lifecycle before user secrets')
 s.comp('XTAL','Y1','ABS07-32.768KHZ-6-T',317.5,190.5,{'1':'LSE_IN','2':'LSE_OUT'},'Crystal:Crystal_SMD_3215-2Pin_3.2x1.5mm',note='LSE for VBAT tamper clock, not OTP. 9pF loading provisional; qualify startup/drive across temperature')
 passive(s,'C','9pF C0G - tune',363.22,185.42,'LSE_IN','GND');passive(s,'C','9pF C0G - tune',398.78,185.42,'LSE_OUT','GND')
 s.comp('PWR','#FLG6','PWR_FLAG',381,91.44,{'1':'VDDA'})
 s.text('STiRoT must verify protected firmware. TrustZone/GTZC must deny\nnonsecure access to FP, SE, LCD, buttons, timers, DMA and key memory.\nNo raw cryptography or arbitrary display commands at secure gateway.\nSE_EN or a sensor MATCH wire is NOT fingerprint authentication.\nSeparate2.2uF on VCAP1/2; no external load on VCAP.',129,258)
 af={'PA5':'SPI1_SCK AF5','PA6':'SPI1_MISO AF5','PA7':'SPI1_MOSI AF5','PB13':'SPI2_SCK AF5','PC2':'SPI2_MISO AF5','PB15':'SPI2_MOSI AF5','PB6':'I2C1_SCL AF4','PB7':'I2C1_SDA AF4','PA8':'I2C3_SCL AF4','PC9':'I2C3_SDA AF4','PC10':'SPI3_SCK AF6','PC11':'SPI3_MISO AF6','PC12':'SPI3_MOSI AF6','PA11':'USB_DM AF10','PA12':'USB_DP AF10','PC1':'TAMP_IN3 dedicated','PC13':'TAMP_IN1 dedicated/VBAT'}
 with (ROOT/'pinout.csv').open('w') as f:
  w=csv.writer(f,lineterminator='\n');w.writerow(['pin','gpio','net','function'])
  for num,net in sorted(nets.items(),key=lambda p:int(p[0])):w.writerow([num,pm[num],net,af.get(pm[num],'power / GPIO / dedicated')])
def storage():
 s=sheet('04-se-storage','Secure element and encrypted storage - no OTP / wall-clock RTC')
 s.comp('SE','U5','SE050E2HQ1/Z01Z3Z',78.74,83.82,{'1':'GND','9':'SE_SDA','10':'SE_SCL','11':'SE_EN','12':'+3V3','14':'GND','15':'SE_CORE','17':'GND','18':'SE_CORE','19':'GND','21':'GND'},'OpenKeys:SE050_SOT1969-1_HX2QFN20',note='VOUT to local VCC only. Per-device pairing, persistent attempts and anti-rollback need validated provisioning')
 s.comp('FLASH','U6','W25Q64JVSSIQ',236.22,83.82,named('FLASH',{'VCC':'+3V3','GND':'GND','~{CS}':'FLASH_CS_N','CLK':'FLASH_SCK','DI/IO_{0}':'FLASH_MOSI','DO/IO_{1}':'FLASH_MISO','~{WP}/IO_{2}':'FLASH_WP_N','~{HOLD}/~{RESET}/IO_{3}':'FLASH_HOLD_N'}))
 for i,(a,b,v) in enumerate([('+3V3','SE_SCL','4.7k'),('+3V3','SE_SDA','4.7k'),('SE_EN','GND','100k'),('+3V3','FLASH_CS_N','47k'),('+3V3','FLASH_WP_N','47k'),('+3V3','FLASH_HOLD_N','47k'),('+3V3','CHG_STAT1_N','10k'),('+3V3','CHG_STAT2_N','10k')]):passive(s,'R',v,30.48+i*48.26,154.94,a,b)
 caps(s,[('100nF','+3V3','GND'),('100nF','SE_CORE','GND'),('1uF','SE_CORE','GND'),('100nF','+3V3','GND')],50.8,215.9)
 s.text('Each user security operation: fresh authenticated device FP + confirmation.\nSensitive administration also needs device PIN. PIN never replaces FP.\nPrecharge attempts durably; uncertain writes deny. AEAD does not alone prevent rollback.\nSE certification does not certify MCU, PCB, biometric path or complete product.',184,264)
def nfc():
 s=sheet('05-nfc','NFC Type4 card interface - battery-powered trusted fingerprint UI')
 s.comp('NFC','U7','FM11NC08S (DFN10 + EP)',114.3,91.44,{'1':'+3V3','2':'NFC_VOUT','3':'ANT1','4':'ANT2','5':'GND','6':'NFC_CS_N','7':'NFC_SCK','8':'NFC_MOSI','9':'NFC_MISO','10':'NFC_IRQ_N','11':'GND'},'OpenKeys:FM11NC08S_DFN10_3x3_EP1.7x2.4',note='Original candidate lands from Fudan v1.0 p9: package3x3,pitch0.5,EP rotated1.7x2.4; confirm supplier revision, sample orientation, paste and DFM')
 s.comp('CONN2','J4','Integral 13.56MHz PCB loop',287.02,76.2,{'1':'ANT1','2':'ANT2'},'OpenKeys:NFC_PCB_Loop_4T_62x15',note='Integral copper4-turn coil; no purchased connector. Separate RF-only region. Initial matching model must be measured in assembled shell.')
 caps(s,[('100nF','+3V3','GND'),('1uF','+3V3','GND'),('100nF 10V','NFC_VOUT','GND')],53.34,172.72)
 passive(s,'C','56pF C0G 50V initial match',154.94,172.72,'ANT1','ANT2')
 passive(s,'R','47k',226.06,172.72,'+3V3','NFC_IRQ_N');passive(s,'R','47k',264.16,172.72,'+3V3','NFC_CS_N')
 s.text('NFC VOUT isolated from SYS/BAT/+3V3. No assumed RF-powered full device.\nFP/LCD/CTAP NFC require charged battery and device ON.\nRF region: tuned coil only, no cell, metal shield or ground planes.\nMobile management needs authenticated APDUs and native OS integration.\nPhone biometrics or a tap never replace device fingerprint verification.',190,241)
def trusted_ui():
 s=sheet('06-trusted-ui','White-backlit16x2 LCD, authenticated fingerprint and contextual buttons')
 s.comp('LCD','U8','NHD-C0216CiZ-FSW-FBW-3V3',88.9,71.12,{'1':'LCD_RESET_N','2':'LCD_SCL','3':'LCD_SDA','4':'GND','5':'LCD_3V0','6':'LCD_VOUT','7':'LCD_C1P','8':'LCD_C1N','A':'LCD_LED_A','K':'LCD_LED_K'},'OpenKeys:LCD_NHD_C0216CiZ_P1.5mm',note='Newhaven rev21: I2C16x2 white LED; logic2.7..3.3V on dedicated3.0V rail')
 s.comp('LDO','U9','TLV70030DDCR',198.12,63.5,named('LDO',{'IN':'+3V3','EN':'+3V3','GND':'GND','OUT':'LCD_3V0'}))
 s.comp('CONN12','J5','FPC2534AM / FH12-12S-0.5SH(55)',320.04,73.66,{'1':'GND','2':None,'3':None,'4':'FP_IRQ','5':'FP_MOSI','6':'FP_MISO','7':'FP_SCK','8':'FP_CS_N','9':'FP_RESET_N','10':'FP_3V3','11':'GND','12':'GND'},'Connector_FFC-FPC:Hirose_FH12-12S-0.5SH_1x12-1MP_P0.50mm_Horizontal',note='100026230; bottom-contact0.5mm/0.3mm flex. Authenticated GCM + durable operation tags; no plaintext MATCH GPIO')
 passive(s,'FB','600R@100MHz',373.38,127,'+3V3','FP_3V3')
 caps(s,[('1uF','+3V3','GND'),('1uF','LCD_3V0','GND'),('1uF','LCD_VOUT','LCD_3V0'),('100nF','LCD_C1P','LCD_C1N'),('10uF','FP_3V3','GND'),('100nF','FP_3V3','GND')],30.48,132.08)
 for i,(a,b,v) in enumerate([('LCD_3V0','LCD_SCL','4.7k'),('LCD_3V0','LCD_SDA','4.7k'),('LCD_RESET_N','GND','100k'),('FP_3V3','FP_CS_N','47k'),('FP_IRQ','GND','100k'),('FP_RESET_N','GND','100k')]):passive(s,'R',v,30.48+i*58.42,172.72,a,b)
 passive(s,'R','33R 1%',50.8,215.9,'+3V3','LCD_LED_A');s.comp('NMOS','Q3','2N7002',109.22,215.9,{'1':'LCD_BACKLIGHT','2':'GND','3':'LCD_LED_K'});passive(s,'R','100k',154.94,215.9,'LCD_BACKLIGHT','GND')
 for i,net in enumerate(['BTN1_N','BTN2_N','BTN3_N','BTN4_N']):
  xx=210.82+i*50.8;s.comp('SW','SW'+str(i+2),'BUTTON '+str(i+1),xx,215.9,{'1':net,'2':'GND'},'Button_Switch_SMD:SW_SPST_TL3342');passive(s,'R','47k',xx,241.3,'+3V3',net);passive(s,'C','10nF',xx,269.24,net,'GND')
 s.comp('PWR','#FLG5','PWR_FLAG',373.38,40.64,{'1':'FP_3V3'})
 s.text('ENGLISH16x2; page full identity before confirmation.\nPIN: short1..4=symbol; hold4=erase; hold2=cancel.\nMenu:1=prev,2=next,3=confirm,4=cancel. No host injection.\nBacklight about10mA via33R; default OFF.',95,274,1.05)
def fault_tamper():
 s=sheet('07-fault-tamper','Independent reset, battery-backed case tamper and SE interlock')
 s.comp('AON','U13','MCP1700T-3002E/TT',337.82,40.64,named('AON',{'VI':'BAT','VO':'TAMP_3V0','GND':'GND'}),note='Always-on3.0V supplies VBAT/case circuit when main switch is OFF; qualify reverse injection and backup switchover')
 caps(s,[('1uF','BAT','GND'),('1uF','TAMP_3V0','GND')],335.28,96.52)
 s.comp('SUP','U10','TPS3702CX33DDCR',99.06,78.74,named('SUP',{'SENSE':'+3V3','SET':'+3V3','VDD':'SYS','GND':'GND','UV':'NRST','OV':'NRST'}),'Package_TO_SOT_SMD:TSOT-23-6',note='SET high: nominal UV3.17V/OV3.43V; open-drain reset independent of ADC/software. Not nanosecond fault protection')
 s.comp('SW','SW6','LID CLOSED contact',246.38,68.58,{'1':'TAMP_3V0','2':'CASE_CLOSED'},'Button_Switch_SMD:SW_SPST_TL3342',note='Lid actuator closes contact; open wire pulls low => independent reset. Travel/contact reliability unvalidated')
 passive(s,'R','100k',292.1,68.58,'CASE_CLOSED','GND')
 s.comp('BUF','U11','SN74LVC1G07DCKR',246.38,124.46,{'1':None,'2':'CASE_CLOSED','3':'GND','4':'NRST','5':'+3V3'},'Package_TO_SOT_SMD:SOT-353_SC-70-5',note='Open-drain case-open reset; cannot drive reset high against supervisor')
 s.comp('AND','U12','SN74LVC1G08DCKR',99.06,172.72,{'1':'NRST','2':'SE_ENABLE_REQUEST','3':'GND','4':'SE_EN','5':'+3V3'},'Package_TO_SOT_SMD:SOT-353_SC-70-5',note='Fault/reset disables SE_EN independently of request. Not a fingerprint authorization gate')
 passive(s,'R','100k',154.94,172.72,'SE_ENABLE_REQUEST','GND')
 caps(s,[('100nF 10V','SYS','GND'),('100nF','+3V3','GND'),('100nF','+3V3','GND')],246.38,177.8)
 s.text('Reset invalidates volatile grants; reboot must verify firmware and require fresh FP.\nPC13=case input; PC1/TAMP_IN3 observes reset/fault line.\nInternal BOR/PVD, clock checks and persistent precharged attempts also required.\nProduction debug lifecycle closed; no populated external debug connector.\nVBAT+LSE must retain tamper with main OFF; full backup loss must lock, not initialize.\nMeasure reset/ENA latency, leakage and injection before security release.',187,255)
def input_guard():
 s=sheet('08-input-guard','Independent VBUS overvoltage cutoff, reverse blocking and limited inrush')
 s.comp('EFUSE','U14','TPS259470LRPWR',144.78,86.36,named('EFUSE',{'EN_UVLO':'USB_GUARD_EN','OVLO':'USB_GUARD_OV','AUXOFF':None,'FLT_N':'USB_GUARD_FAULT_N','IN':'VBUS_FUSED','OUT':'USB_5V','DVDT':'USB_GUARD_DVDT','GND':'GND','ILM':'USB_GUARD_ILM','ITIMER':None}),'OpenKeys:TPS25947_RPW0010A',note='Adjustable OVLO, latch-off variant for latched faults, true reverse-current blocking. Follow datasheet fault-specific recovery; do not infer all OV events latch. Input abs28V is not a system surge rating.')
 for i,(v,a,b) in enumerate([('22.1k 1%','VBUS_FUSED','USB_GUARD_EN'),('10k 1%','USB_GUARD_EN','GND'),('374k 0.1%','VBUS_FUSED','USB_GUARD_OV'),('100k 0.1%','USB_GUARD_OV','GND'),('6.49k 1%','USB_GUARD_ILM','GND'),('47k','+3V3','USB_GUARD_FAULT_N')]):passive(s,'R',v,35.56+i*63.5,157.48,a,b)
 s.comp('ZEN','D4','BZT52-C5V1,115',58.42,88.9,{'1':'USB_GUARD_EN','2':'GND'},'Diode_SMD:D_SOD-123',note='Protect EN/UVLO absolute maximum6.5V; not tied directly to raw VBUS. Verify leakage/temp and dynamic clamp.')
 s.comp('TVS','D5','SMF16A',302.26,86.36,{'1':'VBUS_FUSED','2':'GND'},'Diode_SMD:D_SOD-123F',note='Input surge TVS AFTER F1. Nominal datasheet clamp26V vs eFuse28Vabs is not an IEC guarantee; qualify actual diode supplier, layout and I2t.')
 caps(s,[('100nF 50V','VBUS_FUSED','GND'),('1nF C0G','USB_GUARD_DVDT','GND')],246.38,223.52)
 s.text('OVLO nominal1.2V x(1+374k/100k)=5.688V. Component/leakage corners in protection-calculations.json.\nUVLO divider22.1k/10k: about3.85V nominal. EN is separately clamped, never raw VBUS.\n6.49k ILM: about0.51A protection threshold; BQ charger still sets100mA input budget.\n1nF DVDT: about2V/ms, about2.5ms ramp to5V; measure actual inrush with complete load.\nF1 -> input TVS -> eFuse -> output TVS/ESD reference/charger/sense.\nIndependent cutoff is a circuit provision; response overshoot, CC/data faults and thermal/surge tests remain BLOCKED.',195,263)
def lcd_guard():
 s=sheet('09-display-reset','Independent physical LCD clearing and level-safe reset')
 s.comp('BUF','U15','SN74LVC1G07DCKR',106.68,91.44,{'1':None,'2':'NRST','3':'GND','4':'LCD_RESET_N','5':'+3V3'},'Package_TO_SOT_SMD:SOT-353_SC-70-5',note='Open drain: NRST low clears LCD independently of MCU firmware;3.0V pull-up prevents3.3V overdrive')
 passive(s,'R','10k',236.22,91.44,'LCD_3V0','LCD_RESET_N',note='PC3 MUST be open-drain. With R25=100k the high level is about2.73V, above LCD VIH; never drive3.3V push-pull')
 passive(s,'C','100nF',106.68,157.48,'+3V3','GND')
 s.text('NRST asserts from supervisor, case-open buffer or MCU reset.\nU15 drives LCD_RESET_N low without waiting for firmware startup.\nPC3 can also pull low on lock, expiry, cancel or display fault.\nPC3 open-drain; SCL/SDA open-drain on3.0V pull-ups.\nNo host raw-display command. Hardware reset is not a substitute for\nfingerprint gating of display content and authenticated transactions.\nValidate reset minimum width and complete physical LCD clearing.',225,220)
def save():
 for ss in S:ss.save()
 root=['kicad_sch',['version',20250114],['generator',q('eeschema')],['uuid',q(nuid('OpenKeys'))],['paper',q('A3')],['title_block',['title',q('OpenKeys R0.4 - Engineering review')],['date',q('2026-11-08')],['rev',q('0.4')],['company',q('Jay-Void-BA')]],['lib_symbols']]
 root.append(['text',q('OPENKEYS R0.4 - ONE USB-C + NFC / WHITE 16x2 LCD\nFingerprint mandatory; PIN additional only\nPCB candidate; qualification required before secret provisioning'),['at',190,28,0],effects(2),['uuid',q(nuid('root/title'))]])
 for i,ss in enumerate(S):
  x=20+(i%3)*128;y=65+(i//3)*67
  root.append(['sheet',['at',x,y],['size',115,42],['stroke',['width',0.1524],['type','default']],['fill',['color',0,0,0,0]],['uuid',q(ss.uid)],['property',q('Sheetname'),q(ss.name),['at',x,y-1,0],effects(1.5)],['property',q('Sheetfile'),q(ss.name+'.kicad_sch'),['at',x,y+44,0],effects()],['instances',['project',q('OpenKeys'),['path',q('/'+nuid('OpenKeys')),['page',q(str(i+2))]]]]])
 root.append(['sheet_instances',['path',q('/'),['page',q('1')]]])
 (CAD/'OpenKeys.kicad_sch').write_text(pretty(root)+'\n')
 used={key:lib for ss in S for key,lib in ss.used.items()}
 (CAD/'OpenKeys.kicad_sym').write_text(pretty(['kicad_symbol_lib',['version',20241209],['generator',q('kicad_symbol_editor')],*used.values()])+'\n')
 (CAD/'sym-lib-table').write_text('(sym_lib_table (version 7) (lib (name "OpenKeys") (type "KiCad") (uri "${KIPRJMOD}/OpenKeys.kicad_sym") (options "") (descr "R0.4 engineering symbols")))\n')
 project_path=CAD/'OpenKeys.kicad_pro'
 project=json.loads(project_path.read_text()) if project_path.exists() else {'meta':{'filename':'OpenKeys.kicad_pro','version':1}}
 project.setdefault('text_variables',{})['DESIGN_STATUS']='BOARD_CANDIDATE_NOT_QUALIFIED'
 project_path.write_text(json.dumps(project,indent=2)+'\n')
 comps=[c for ss in S for c in ss.components]
 with (ROOT/'BOM.csv').open('w') as f:
  w=csv.DictWriter(f,fieldnames=['ref','value','footprint','sheet','dnp','note'],lineterminator='\n');w.writeheader();w.writerows({k:c[k] for k in w.fieldnames} for c in comps)
 (ROOT/'review/connections.json').write_text(json.dumps(comps,indent=2)+'\n')
 print(f'R0.4: {len(S)+1} sheets, {len(comps)} BOM positions')
if __name__=='__main__':
 usb();power();mcu();storage();nfc();trusted_ui();fault_tamper();input_guard();lcd_guard();save()
