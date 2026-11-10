#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""Independent DC corner calculations; explicitly not transient qualification."""
from pathlib import Path
import itertools,json
R=Path(__file__).resolve().parents[1]
# TI SLVSFC9C OVLO table, full specified temperature range. Divider tolerance0.1%.
limits=[]
for rt,rb,ref,leak in itertools.product([374000*.999,374000*1.001],[100000*.999,100000*1.001],[1.183,1.223],[-.1e-6,.1e-6]):
 limits.append(ref*(1+rt/rb)+leak*rt)
lo,hi=min(limits),max(limits)
assert 5.5<lo<hi<6.0
# ILM resistor must remain inside the documented549..6650ohm range, including tolerance.
assert 549<6490*.99<6490*1.01<6650
report={'revision':'0.5-input-guard','source':'TI TPS25947 SLVSFC9C, pin/electrical tables and equation4','ovlo_nominal_v':1.2*(1+374000/100000),'ovlo_dc_corners_v':[lo,hi],'ovlo_inputs':{'rt_ohm':374000,'rb_ohm':100000,'r_tolerance_fraction':.001,'reference_v':[1.183,1.223],'leakage_a':[-.1e-6,.1e-6]},'uvlo_nominal_v':1.2*(1+22100/10000),'uvlo_corner_qualified':False,'uvlo_limits':'Zener leakage vs temperature, enable threshold, divider tolerance and ramp require real qualification. Nominal value is not a guaranteed bound.','ilm_resistor_ohm':[6490*.99,6490*1.01],'current_limit_typical_a_approx':.5*6650/6490,'guard_current_limit_is_not_usb_consumption_limit':True,'dvdt_nominal_v_per_ms':2000/1000,'ramp_to_5v_ms_nominal':5/(2000/1000),'qualification':{'dc_math_only':True,'transient_overshoot':False,'tvspptc_i2t_and_temperature':False,'reverse_power_off_and_pin_backfeed':False,'usb_inrush_suspend_and_total_current':False,'protection_against_arbitrary_voltage_or_energy':False},'manufacturing_release':False}
(R/'review/protection-calculations.json').write_text(json.dumps(report,indent=2)+'\n')
print(f'OVLO DC corner calculation {lo:.4f}..{hi:.4f} V; transient/thermal and release NOT qualified')
