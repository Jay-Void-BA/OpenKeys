#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""Static DC corners only; not a transient/fault/power-budget qualification."""
import csv
import itertools
import json
from pathlib import Path

root = Path(__file__).resolve().parents[1]
bom = {row['ref']: row for row in csv.DictReader((root / 'BOM.csv').open())}

def resistance(ref):
    value = bom[ref]['value']
    assert value.endswith(' 0.1%'), (ref, value)
    return float(value.split()[0].removesuffix('k')) * 1000

def corners(top, bottom, references):
    values = [v * (1 + rt / rb) + current * rt
              for v, rt, rb, current in itertools.product(
                  references, (top * .999, top * 1.001),
                  (bottom * .999, bottom * 1.001), (-25e-9, 25e-9))]
    return {'min': min(values), 'max': max(values)}

assert bom['U22']['value'] == 'TPS3700DDCR'
assert bom['U21']['value'] == 'TCA9517ADGKR'
assert bom['U23']['value'] == 'SN74LVC1G07DCKR'
uv = {edge: corners(resistance('R49'), resistance('R50'), thresholds)
      for edge, thresholds in [('rising', (.396, .404)), ('falling', (.387, .400))]}
ov = {edge: corners(resistance('R51'), resistance('R52'), thresholds)
      for edge, thresholds in [('rising', (.396, .404)), ('falling', (.387, .400))]}
drop = .050
lcd_min = uv['falling']['min'] - drop
lcd_max = ov['rising']['max']
assert lcd_min > 3.3 and lcd_max < 5.0
assert lcd_min > 3.333  # A/B voltage ordering also has static margin.
assert uv['rising']['max'] < 3.6  # Deliberate early battery cutoff/restart.
assert ov['falling']['min'] > 4.5  # Normal SYS can recover after an OV event.
report = {
    'schema': 1, 'revision': '0.5', 'status': 'STATIC_DC_CORNERS_ONLY',
    'source': 'TI TPS3700 SBVS187G electrical characteristics, TPS63031 fixed PWM bounds',
    'resistor_tolerance_fraction': .001, 'input_bias_bound_a': 25e-9,
    'positive_reference_v': [.396, .404], 'negative_reference_v': [.387, .400],
    'sys_uv_v': uv, 'sys_ov_v': ov,
    'main_rail_fixed_pwm_v': [3.267, 3.333],
    'assumed_total_switch_and_harness_drop_max_v': drop,
    'static_lcd_supply_v': [lcd_min, lcd_max],
    'conditions': [
        'Actual switched-rail/harness drop <=50mV across load, temperature and units',
        'Main regulator forced PWM; DC bounds do not include ripple or transients',
        'TPS3700 powered and settled; startup services held off for at least500ms',
        'Comparator, reset, buffer and load-switch propagation are finite and must be measured',
        'Fault pulses, active injection, input-current and thermal margins are not modeled',
        'LCD capacitance, powered-off leakage, bus pullups and visible clearing must be measured',
    ],
    'physical_qualification': False, 'production_release': False,
}
(root / 'review/lcd-voltage-window.json').write_text(json.dumps(report, indent=2) + '\n')
print(f'Static LCD window {lcd_min:.4f}..{lcd_max:.4f}V under stated conditions; physical tests NOT_RUN')
