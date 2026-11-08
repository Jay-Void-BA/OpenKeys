#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""Reproducible passive/geometry simulations, not vendor transistor models.

Requires Python3 and ngspice47. No scientific Python dependency is needed.
The line-integral antenna estimate is deliberately reported as an estimate.
It is not a full-wave simulation or a claim of NFC interoperability.
"""
import argparse, csv, hashlib, itertools, json, math, re, subprocess
from pathlib import Path

R=Path(__file__).resolve().parents[1]
SIM=R/'simulation'

def coil(step=.125):
 # Exact centerline written by footprints.py, including the B.Cu underpass
 # and a short closing return through the chip/matching network.
 points=[(0.,0.,0.),(0.,3.,0.)]
 for k in range(4):
  d=.9*k;points += [(62-d,3+d,0),(62-d,18-d,0),(d,18-d,0),(d,3+d+.9,0)]
  if k<3:points += [(d+.9,3+d+.9,0)]
 points += [(points[-1][0],points[-1][1],-1),(5.,0.,-1),(5.,0.,0),(0.,0.,0)]
 centers=[];dls=[];length=0.
 for a,b in zip(points,points[1:]):
  delta=tuple((y-x)*1e-3 for x,y in zip(a,b));n=math.ceil(math.sqrt(sum(x*x for x in delta))/(step*.001))
  length+=math.sqrt(sum(x*x for x in delta))
  for k in range(n):
   centers.append(tuple(x*.001+d*(k+.5)/n for x,d in zip(a,delta)));dls.append(tuple(x/n for x in delta))
 # GMD regularization of finite rectangular copper cross-section. It makes
 # the otherwise singular Neumann integral finite. Skin/proximity effects
 # and nonlinear chip input impedance are not solved by this model.
 gmd=.2235*(.5+.035)*.001
 total=0.
 for i,(a,da) in enumerate(zip(centers,dls)):
  for j in range(i,len(centers)):
   b,db=centers[j],dls[j]
   dot=sum(x*y for x,y in zip(da,db))
   if dot==0:continue
   distance=math.sqrt(sum((x-y)**2 for x,y in zip(a,b))+gmd*gmd)
   total+=(1 if i==j else 2)*dot/distance
 return total*1e-7,length,len(centers)

def run_spice(executable, name, netlist):
 path=SIM/(name+'.cir');path.write_text(netlist)
 r=subprocess.run([executable,'-b',str(path)],cwd=SIM,text=True,capture_output=True,check=False)
 log='\n'.join(line.rstrip() for line in (r.stdout+r.stderr).splitlines())+'\n'
 for table in SIM.glob('*.txt'):
  table.write_text('\n'.join(line.rstrip() for line in table.read_text().splitlines())+'\n')
 # Keep model failures visible, including warnings. Never suppress them.
 (SIM/(name+'.log')).write_text(log)
 if r.returncode or re.search(r'(?im)^\s*(warning|error|fatal)\b',log):
  raise RuntimeError(name+' failed or emitted a warning; inspect '+str(SIM/(name+'.log')))
 return log

def main():
 parser=argparse.ArgumentParser();parser.add_argument('--ngspice',default='ngspice');a=parser.parse_args()
 SIM.mkdir(exist_ok=True)
 # Both discretizations are retained to expose numerical convergence.
 lc,length,nc=coil(.25);lf,_,nf=coil(.125)
 assert abs(lf-lc)/lf<.02
 capacitance=50e-12+56e-12+5e-12
 resonance=1/(2*math.pi*math.sqrt(lf*capacitance))
 ac=f'''OpenKeys integral antenna - LINEAR ESTIMATE ONLY
* FM11NC08 specified nominal internal50pF; parasitic5pF is an assumption.
Itest 0 rf DC 0 AC 1u
Rloss rf lnode 2
Lcoil lnode 0 {lf:.12g}
Cchip rf 0 50p
Cmatch rf 0 56p
Cparasitic rf 0 5p
Rchip rf 0 5k
.control
set noaskquit
set wr_singlescale
set wr_vecnames
ac dec 400 5Meg 30Meg
let magnitude=mag(v(rf))
wrdata antenna-ac.txt magnitude
quit
.endc
.end
'''
 run_spice(a.ngspice,'antenna',ac)
 reset='''OpenKeys reset RC - independent open-drain input, not supervisor model
* R11=10k, C19=10n. Driver10ohm and input20pF are explicit assumptions.
Vrail supply 0 3.3
Rpull supply nrst 10k
Creset nrst 0 10n
Cinput nrst 0 20p
Vfault fault 0 PULSE(0 3.3 1m 1n 1n 20u 2m)
Sfault nrst 0 fault 0 reset_switch
.model reset_switch SW(Ron=10 Roff=1e12 Vt=1.65 Vh=0)
.control
set noaskquit
set wr_singlescale
set wr_vecnames
tran 20n 1.5m
meas tran nrst_fall WHEN v(nrst)=0.99 FALL=1
meas tran nrst_recover WHEN v(nrst)=2.31 RISE=1
wrdata reset-transient.txt v(nrst) v(fault)
quit
.endc
.end
'''
 log=run_spice(a.ngspice,'reset',reset)
 measures={k:float(v) for k,v in re.findall(r'(nrst_fall|nrst_recover)\s*=\s*([0-9.eE+-]+)',log)}
 assert .001<measures['nrst_fall']<.001001
 assert .00113<measures['nrst_recover']<.00116
 corners=[]
 for lt,cc in itertools.product([.8,1.2],[40e-12+56e-12+2e-12,60e-12+56e-12+12e-12]):
  corners.append(1/(2*math.pi*math.sqrt(lf*lt*cc)))
 report={'revision':'0.4','simulator':subprocess.check_output([a.ngspice,'--version'],text=True).splitlines()[1].strip(),
 'antenna':{'method':'finite-cross-section Neumann centerline estimate','segments':[nc,nf],'step_mm':[.25,.125],
 'L_h':[lc,lf],'numerical_delta_fraction':abs(lf-lc)/lf,'estimated_length_m':length,'trace_width_mm':.5,'copper_um':35,
 'dc_resistance_ohm_at_20C_estimate':1.724e-8*length/(.0005*.000035),'fm11_internal_nominal_pf':50,'external_initial_pf':56,'assumed_parasitic_pf':5,
 'nominal_resonance_hz':resonance,'illustrative_corner_resonance_hz':[min(corners),max(corners)],
 'corner_assumptions':'L estimate +/-20%; chip40..60pF and parasitic2..12pF are exploration assumptions, not manufacturer tolerance bounds',
 'qualified':False},'reset_rc':{'r_ohm':10000,'c_f':10e-9,'assumed_pin_c_f':20e-12,'assumed_driver_ohm':10,'threshold_measurements_s':measures,
 'supervisor_delay_included':False,'tps3702_typical_hl_delay_s':19e-6,'tps3702_guaranteed_max_delay_available':False},
 'scope':'Passive network and approximate inductance only; no IC switcher, battery chemistry, EMC, ESD, SCA or fault-injection qualification',
 'manufacturing_security_release':False}
 (R/'review/simulation.json').write_text(json.dumps(report,indent=2)+'\n')
 print(f'SPICE and geometry checks passed; L estimate {lf*1e6:.3f}uH, resonance {resonance/1e6:.3f}MHz; physical RF/security qualification pending')
if __name__=='__main__':main()
