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
if __name__=='__main__':generate()
