#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""Bounds under EXPLICIT qualification assumptions; not a TI transistor model."""
import itertools
import json
import math
from pathlib import Path

R = Path(__file__).resolve().parents[1]
# Includes the physical module, harness and PCB. DFRobot does NOT specify this
# capacitance bound: measure it before using the following discharge result.
cap_f = 100e-6
bleeder_max = 1000 * 1.01
off_s = .5
initial_v = 3.465
residual = initial_v * math.exp(-off_s / (bleeder_max * cap_f))
assert residual < .1
cases = []
for clear, previous_q, clock_edge in itertools.product([False, True], repeat=3):
    q = False if clear else (True if clock_edge else previous_q)
    for delayed in [False, True]:
        bus = q and delayed
        assert not clear or not (q or bus)
        if not previous_q and not clock_edge:
            assert not q and not bus
        cases.append(dict(clear_asserted=clear, previous_q=previous_q,
                          new_clock_edge=clock_edge, delay_ready=delayed,
                          power_on=q, bus_connected=bus))
report = {
    'revision': '0.5', 'status': 'ASSUMPTION_BOUNDED_MODEL_ONLY',
    'assumed_total_lcd_capacitance_max_f': cap_f,
    'bleeder_max_ohm': bleeder_max, 'off_hold_s': off_s,
    'assumed_initial_max_v': initial_v, 'ideal_no_injection_residual_v': residual,
    'truth_table': cases,
    'qualification_conditions': [
        'Actual module plus PCB capacitance <=100uF over bias/temperature/units',
        'No SCL/SDA/ESD/harness injection maintains the switched rail',
        'Reset/PC3 clears latch at power ramps and brownouts; no unwanted CLK edge',
        'Both bus switches disconnect promptly before rail decay',
        'Measured visible text disappears after faults without automatic repower',
        '3.3V nominal supply and cable meet real LCD operating limits at corners',
        'Actual latch, switch, threshold and propagation timings validated',
    ],
    'physical_qualification': False, 'production_release': False,
}
(R / 'review/display-power-model.json').write_text(json.dumps(report, indent=2) + '\n')
print(f'Logical/discharge model only: residual {residual:.4f}V under stated assumptions; physical tests NOT_RUN')
