#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""SE050 SOT1969-1 geometry from NXP package drawing, pages 4-6.

Coordinates are facts transcribed into an original KiCad footprint. No vendor
PDF/image or unverified generic QFN land pattern is embedded. DFM/assembly
validation remains required before manufacturing release.
"""
from pathlib import Path
import pcbnew as p
ROOT=Path(__file__).resolve().parents[1]
LIB=ROOT/'kicad/OpenKeys.pretty'
def v(x,y):return p.VECTOR2I(p.FromMM(x),p.FromMM(y))
def shape(fp,a,b,layer,width=.05):
 s=p.PCB_SHAPE();s.SetShape(p.SHAPE_T_RECT);s.SetStart(v(*a));s.SetEnd(v(*b));s.SetLayer(layer);s.SetWidth(p.FromMM(width));fp.Add(s)
 return s

def generate():
 LIB.mkdir(exist_ok=True)
 fp=p.FOOTPRINT(None);fp.SetFPID(p.LIB_ID('OpenKeys','SE050_SOT1969-1_HX2QFN20'))
 fp.SetReference('REF**');fp.SetValue('SE050_SOT1969-1_HX2QFN20');fp.SetAttributes(p.FP_SMD)
 fp.SetLibDescription('NXP SOT1969-1 HX2QFN20; 0.4mm pitch; 0.725x0.2 lands; 1.9mm EP; 4x0.75mm paste windows; NXP 2019-05-23 figures 3-5')
 fp.Reference().SetPosition(v(0,-2.8));fp.Reference().SetTextSize(v(.8,.8));fp.Reference().SetTextThickness(p.FromMM(.12))
 fp.Value().SetPosition(v(0,2.8));fp.Value().SetLayer(p.F_Fab);fp.Value().SetTextSize(v(.6,.6));fp.Value().SetTextThickness(p.FromMM(.1))
 for n in range(1,21):
  side=(n-1)//5;i=(n-1)%5
  x,y=[(-1.538,-.8+.4*i),(-.8+.4*i,1.538),(1.538,.8-.4*i),(.8-.4*i,-1.538)][side]
  pad=p.PAD(fp);pad.SetNumber(str(n));pad.SetAttribute(p.PAD_ATTRIB_SMD);pad.SetShape(p.PAD_SHAPE_RECT);pad.SetPosition(v(x,y));pad.SetSize(v(.725,.2) if side in (0,2) else v(.2,.725))
  ls=p.LSET()
  for layer in [p.F_Cu,p.F_Mask,p.F_Paste]:ls.AddLayer(layer)
  pad.SetLayerSet(ls);pad.SetLocalSolderMaskMargin(p.FromMM(.05));fp.Add(pad)
 ep=p.PAD(fp);ep.SetNumber('21');ep.SetAttribute(p.PAD_ATTRIB_SMD);ep.SetShape(p.PAD_SHAPE_RECT);ep.SetPosition(v(0,0));ep.SetSize(v(1.9,1.9))
 ls=p.LSET();ls.AddLayer(p.F_Cu);ls.AddLayer(p.F_Mask);ep.SetLayerSet(ls);ep.SetLocalSolderMaskMargin(p.FromMM(.05));fp.Add(ep)
 for x in [-.475,.475]:
  for y in [-.475,.475]:
   z=shape(fp,(x-.375,y-.375),(x+.375,y+.375),p.F_Paste,0);z.SetFilled(True)
 shape(fp,(-1.5,-1.5),(1.5,1.5),p.F_Fab)
 shape(fp,(-2.05,-2.05),(2.05,2.05),p.F_CrtYd)
 dot=p.PCB_SHAPE();dot.SetShape(p.SHAPE_T_CIRCLE);dot.SetStart(v(-2.2,-1.45));dot.SetEnd(v(-2.1,-1.45));dot.SetLayer(p.F_SilkS);dot.SetWidth(p.FromMM(.12));fp.Add(dot)
 p.PCB_IO_MGR.PluginFind(p.PCB_IO_MGR.KICAD_SEXP).FootprintSave(str(LIB),fp)
 (ROOT/'kicad/fp-lib-table').write_text('(fp_lib_table (version 7) (lib (name "OpenKeys") (type "KiCad") (uri "${KIPRJMOD}/OpenKeys.pretty") (options "") (descr "OpenKeys verified package geometry")))\n')
 print('Generated SOT1969-1 footprint: 20 lands + grounded EP + four paste windows')
def lcd():
 fp=p.FOOTPRINT(None);fp.SetFPID(p.LIB_ID('OpenKeys','LCD_NHD_C0216CiZ_P1.5mm'))
 fp.SetReference('REF**');fp.SetValue('NHD-C0216CiZ-FSW-FBW-3V3');fp.SetAttributes(p.FP_THROUGH_HOLE)
 fp.SetLibDescription('Original transcription of Newhaven rev21 footprint facts: eight 0.8mm holes at1.5mm; LED A/K1.0mm holes. Pin1 origin. Verify sample before release.')
 for num,x,y,drill,size in [(str(n),-1.5*(n-1),0,.8,1.3) for n in range(1,9)]+[('A',27.45,-15.34,1.0,1.8),('K',27.45,-17.84,1.0,1.8)]:
  pad=p.PAD(fp);pad.SetNumber(num);pad.SetAttribute(p.PAD_ATTRIB_PTH);pad.SetShape(p.PAD_SHAPE_RECT if num=='1' else p.PAD_SHAPE_CIRCLE);pad.SetPosition(v(x,y));pad.SetSize(v(size,size));pad.SetDrillSize(v(drill,drill));pad.SetLayerSet(p.LSET.AllCuMask());ls=pad.GetLayerSet();ls.AddLayer(p.F_Mask);ls.AddLayer(p.B_Mask);pad.SetLayerSet(ls);fp.Add(pad)
 shape(fp,(-30.1,-26.35),(26.7,-7.45),p.F_Fab)
 shape(fp,(-30.6,-26.85),(28.85,1.1),p.F_CrtYd)
 fp.Reference().SetPosition(v(0,2.5));fp.Value().SetPosition(v(0,4));fp.Value().SetLayer(p.F_Fab)
 p.PCB_IO_MGR.PluginFind(p.PCB_IO_MGR.KICAD_SEXP).FootprintSave(str(LIB),fp)
 print('Generated candidate 16x2 white LCD footprint; physical fit not qualified')
def guard():
 # Original transcription of TI RPW0010A land-pattern facts (SLVSFC9C,p73).
 # No central grounded EP: long inner lands are IN5 and OUT6, not ground.
 fp=p.FOOTPRINT(None);fp.SetFPID(p.LIB_ID('OpenKeys','TPS25947_RPW0010A'));fp.SetReference('REF**');fp.SetValue('TPS25947_RPW0010A');fp.SetAttributes(p.FP_SMD)
 fp.SetLibDescription('TI RPW0010A 2x2mm HotRod10; 0.475/0.45 pitch; compound corner lands; IN5/OUT6 long lands. Sample/stencil/DFM qualification pending.')
 def pad(n,x,y,w,h):
  a=p.PAD(fp);a.SetNumber(str(n));a.SetAttribute(p.PAD_ATTRIB_SMD);a.SetShape(p.PAD_SHAPE_ROUNDRECT);a.SetPosition(v(x,y));a.SetSize(v(w,h));a.SetRoundRectRadiusRatio(.05/min(w,h));ls=p.LSET()
  for ly in [p.F_Cu,p.F_Mask,p.F_Paste]:ls.AddLayer(ly)
  a.SetLayerSet(ls);a.SetLocalSolderMaskMargin(p.FromMM(.05));fp.Add(a)
 for n,x,y in [(1,-.9,-.7),(4,-.9,.7),(7,.9,.7),(10,.9,-.7)]:
  pad(n,x,y,.6,.3);pad(n,-.725 if x<0 else .725,-.875 if y<0 else .875,.25,.65)
 for n,x,y in [(2,-.9,-.225),(3,-.9,.225),(8,.9,.225),(9,.9,-.225)]:pad(n,x,y,.6,.25)
 pad(5,-.25,0,.3,2.4);pad(6,.25,0,.3,2.4)
 shape(fp,(-1,-1),(1,1),p.F_Fab);shape(fp,(-1.45,-1.45),(1.45,1.45),p.F_CrtYd)
 fp.Reference().SetPosition(v(0,-2));fp.Value().SetPosition(v(0,2));fp.Value().SetLayer(p.F_Fab)
 p.PCB_IO_MGR.PluginFind(p.PCB_IO_MGR.KICAD_SEXP).FootprintSave(str(LIB),fp)
 print('Generated candidate RPW0010A footprint: ten electrical pins; IN/OUT are not a grounded EP')
def nfc():
 # Fudan FM11NC08 product specification v1.0 p9. Top-side footprint orientation
 # rotated from the bottom-view package drawing: EP1.7x2.4, not2.4x1.7.
 fp=p.FOOTPRINT(None);fp.SetFPID(p.LIB_ID('OpenKeys','FM11NC08S_DFN10_3x3_EP1.7x2.4'));fp.SetReference('REF**');fp.SetValue('FM11NC08S_DN_T_G');fp.SetAttributes(p.FP_SMD)
 fp.SetLibDescription('Fudan v1.0 p9 package:3x3,pitch0.5,EP D1=2.3..2.5/E1=1.6..1.8. Original candidate land pattern rotated EP1.7x2.4;0.7x0.3 lands; assembly qualification pending.')
 for n in range(1,11):
  left=n<=5;y=-1+.5*(n-1) if left else 1-.5*(n-6)
  a=p.PAD(fp);a.SetNumber(str(n));a.SetAttribute(p.PAD_ATTRIB_SMD);a.SetShape(p.PAD_SHAPE_ROUNDRECT);a.SetPosition(v(-1.4 if left else 1.4,y));a.SetSize(v(.7,.3));a.SetRoundRectRadiusRatio(.15);ls=p.LSET()
  for ly in [p.F_Cu,p.F_Mask,p.F_Paste]:ls.AddLayer(ly)
  a.SetLayerSet(ls);a.SetLocalSolderMaskMargin(p.FromMM(.05));fp.Add(a)
 a=p.PAD(fp);a.SetNumber('11');a.SetAttribute(p.PAD_ATTRIB_SMD);a.SetShape(p.PAD_SHAPE_RECT);a.SetPosition(v(0,0));a.SetSize(v(1.7,2.4));ls=p.LSET();ls.AddLayer(p.F_Cu);ls.AddLayer(p.F_Mask);a.SetLayerSet(ls);a.SetLocalSolderMaskMargin(p.FromMM(.05));fp.Add(a)
 for x in [-.425,.425]:
  for y in [-.6,.6]:z=shape(fp,(x-.325,y-.45),(x+.325,y+.45),p.F_Paste,0);z.SetFilled(True)
 shape(fp,(-1.5,-1.5),(1.5,1.5),p.F_Fab);shape(fp,(-2,-1.8),(2,1.8),p.F_CrtYd)
 fp.Reference().SetPosition(v(0,-2.3));fp.Value().SetPosition(v(0,2.3));fp.Value().SetLayer(p.F_Fab)
 p.PCB_IO_MGR.PluginFind(p.PCB_IO_MGR.KICAD_SEXP).FootprintSave(str(LIB),fp)
 print('Generated Fudan-derived candidate NFC land pattern; no production qualification implied')
if __name__=='__main__':generate();lcd();guard();nfc()
