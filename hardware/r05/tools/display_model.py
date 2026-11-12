#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""Bounds under EXPLICIT qualification assumptions; not a TI transistor model."""
import itertools
import csv
import json
import math
from pathlib import Path

R = Path(__file__).resolve().parents[1]
bom = {row['ref']: row for row in csv.DictReader((R / 'BOM.csv').open())}
assert bom['R47']['value'] == '330R 1% 0.25W'
assert bom['U17']['value'] == 'TPS3808G33DBVR'
assert bom['R58']['value'] == '100k 1%'
assert bom['R59']['value'] == '10k 1%'
lcd_dc_max = json.loads((R / 'review/lcd-voltage-window.json').read_text())['static_lcd_supply_v'][1]
# Includes the physical module, harness and PCB. DFRobot does NOT specify this
# capacitance bound: measure it before using the following discharge result.
cap_f = 100e-6
bleeder_max = 330 * 1.01
off_s = .5
# TPS3808 fixed300ms mode:180..420ms. The aggregate turn-off path
# is ASSUMED <=5ms, not a datasheet guarantee for the complete circuit.
hardware_hold_min_s = .180
assumed_turn_off_max_s = .005
hardware_effective_off_s = hardware_hold_min_s - assumed_turn_off_max_s
initial_v = 5.5
# Aggregate maximum powered-off injection from all paths; must be measured.
injection_a = 20e-6
steady_v = injection_a * bleeder_max
residual = steady_v + (initial_v - steady_v) * math.exp(-off_s / (bleeder_max * cap_f))
hardware_residual = steady_v + (initial_v - steady_v) * math.exp(
    -hardware_effective_off_s / (bleeder_max * cap_f))
assert residual < .1 and hardware_residual < .1
cases = []
for clear, previous_q, clock_edge in itertools.product([False, True], repeat=3):
    q = False if clear else (True if clock_edge else previous_q)
    for reset_hold, delayed in itertools.product([False, True], repeat=2):
        effective_q = False if reset_hold else q
        bus = effective_q and delayed
        assert not (clear or reset_hold) or not (effective_q or bus)
        if not previous_q and not clock_edge:
            assert not effective_q and not bus
        cases.append(dict(clear_asserted=clear, reset_hold_active=reset_hold,
                          previous_q=previous_q, new_clock_edge=clock_edge,
                          delay_ready=delayed, power_on=effective_q,
                          bus_connected=bus))
report = {
    'revision': '0.5', 'status': 'ASSUMPTION_BOUNDED_MODEL_ONLY',
    'assumed_total_lcd_capacitance_max_f': cap_f,
    'bleeder_max_ohm': bleeder_max, 'off_hold_s': off_s,
    'assumed_initial_max_v': initial_v,
    'ideal_no_injection_residual_v': initial_v * math.exp(-off_s / (bleeder_max * cap_f)),
    'assumed_injection_max_a': injection_a,
    'residual_with_bounded_injection_v': residual,
    'hardware_reset_delay_s': [.180, .420],
    'assumed_aggregate_turn_off_max_s': assumed_turn_off_max_s,
    'hardware_effective_off_min_s': hardware_effective_off_s,
    'hardware_residual_with_bounded_injection_v': hardware_residual,
    'bleeder_stable_power_max_w': lcd_dc_max**2 / (330 * .99),
    'bleeder_assumed_initial_power_max_w': initial_v**2 / (330 * .99),
    'truth_table': cases,
    'qualification_conditions': [
        'Actual module plus PCB capacitance <=100uF over bias/temperature/units',
        'All SCL/SDA/EN/ESD/harness injection combined <=20uA while off',
        'TPS3808 CT connected to VDD through100k; fixed reset hold180..420ms verified on actual units',
        'Aggregate reset/switch turn-off <=5ms; no CLK edge after hold without trusted Root authorization',
        'Reset/PC3 clears latch at power ramps and brownouts; unpowered ON held low by10k',
        'TCA9517A both bus wires disable promptly before rail decay, including partial power',
        'Measured visible text disappears after faults without automatic repower',
        'SYS voltage monitor, switched LCD rail and cable meet the separate DC window assumptions',
        'Actual latch, switch, threshold and propagation timings validated',
    ],
    'physical_qualification': False, 'production_release': False,
}
(R / 'review/display-power-model.json').write_text(json.dumps(report, indent=2) + '\n')
print(f'Assumption-bounded discharge: hardware {hardware_residual:.4f}V, software {residual:.4f}V; physical tests NOT_RUN')
