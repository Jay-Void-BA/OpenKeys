#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""Netlist-bound backup/ADC allocations and passive RC simulations.

Vendor DC numbers, engineering allocations and ideal logic stimuli are kept
separate. This is not an IC transistor, cell chemistry, FI or SCA simulation.
"""
import argparse
import csv
import hashlib
import itertools
import json
import math
import re
from pathlib import Path
import xml.etree.ElementTree as ET

from simulate import run_spice

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--ngspice', default='ngspice')
    parser.add_argument('--check', action='store_true', help='verify saved model input/source binding without rerunning SPICE')
    args = parser.parse_args()
    bom = {row['ref']: row for row in csv.DictReader((ROOT/'BOM.csv').open())}
    expected = {'U13':'LM66100DCKR', 'U24':'SN74AUP1G99DCUR',
                'U25':'SN74LVC1G14DCKR', 'U26':'SN74LVC1G07DCKR',
                'U27':'SN74AUP2G17DCKR', 'U28':'TMUX1511RWBR',
                'BT1':'Panasonic CR-2032/HFN', 'R61':'10k 1%',
                'R62':'470k 1%', 'R63':'1k 1%', 'R64':'330k 1%',
                'R65':'330k 1%', 'R66':'4.7k 1%', 'R67':'1M 1%',
                'R68':'100k 1%', 'R69':'47k 1%', 'C61':'10nF X7R 16V',
                'C62':'10nF X7R 16V 10%', 'C68':'100pF C0G 50V 5%'}
    assert all(bom[ref]['value']==value for ref,value in expected.items())
    xml = ROOT/'exports/OpenKeys-netlist.xml'
    doc = ET.parse(xml).getroot()
    nets = {n.attrib['name']:{(p.attrib['ref'],p.attrib['pin']) for p in n.findall('node')}
            for n in doc.findall('./nets/net')}
    assert nets['VBAT_BACKUP']=={('U4','1'),('U13','3'),('U13','6'),('C41','1'),('C56','1')}
    assert nets['BATTERY_SENSE']=={('U4','14'),('U28','4'),('R68','1')}
    assert ('C61','1') in nets['BATTERY_DIVIDER']
    assert ('U28','7') in nets['NRST'] and ('U26','4') in nets['NRST']
    inputs={'components':{ref:[row['value'],row['footprint']] for ref,row in bom.items()},
            'nets':{name:sorted(pins) for name,pins in nets.items()}}
    electrical_sha=hashlib.sha256(json.dumps(inputs,sort_keys=True,separators=(',',':')).encode()).hexdigest()
    source_sha=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    report_path=ROOT/'review/backup-tamper-model.json'
    if args.check:
        saved=json.loads(report_path.read_text())
        assert saved['electrical_input_sha256']==electrical_sha, 'backup model electrical inputs changed'
        assert saved['model_source_sha256']==source_sha, 'backup model source changed; rerun simulations'
        assert saved['production_release'] is False and saved['physical_qualification'] is False
        print('Saved backup/ADC model input and source bindings passed; SPICE not rerun')
        return
    # The 1.5uA return and 0.7uA drive allocations deliberately exceed the
    # ordinary GPIO pad + main-powered Schmitt input numbers. VBAT-domain
    # pads, dirty/wet PCB surfaces and brownout behaviour need measurement.
    return_low = 1.5e-6 * 470e3 * 1.01
    drive_low = .7e-6 * 1e6 * 1.01
    assert max(return_low,drive_low)<.88
    budget = []
    for cell,rtc,duty in itertools.product([2.,2.4,3.,3.3],[1.2e-6,3.65e-6],[.5,1.]):
        # Allocations, NOT guaranteed active-TAMP current or usable capacity:
        # 10uA extra active engine, 1uA PCB, half of nominal225mAh usable.
        # DS14121 Table105 sensor buffer max6.5uA is added explicitly;
        # MONEN is required for backup voltage/temperature monitoring.
        base = rtc + 6.5e-6 + 2*.6e-6 + 10e-6 + 1e-6 + .3e-6
        conductance = duty*(1/(470e3*.99)+1/(1e6*.99))
        series = 10e3*1.01
        # LM66100's -250mV turn-on comparator corner is conservatively
        # reserved as headroom, not asserted to be its forward conduction drop.
        vbat = (cell-.250-base*series)/(1+conductance*series)
        current = base+vbat*conductance
        assert 1.2<vbat<3.6
        budget.append({'cell_v':cell,'rtc_bound_a':rtc,'high_duty_allocation':duty,
                       'vbat_v':vbat,'cell_current_a':current,
                       'allocated_capacity_mah':112.5,
                       'illustrative_continuous_backup_days':112.5/(current*1000)/24})
    adc = []
    for rt,rb,rd in itertools.product([330e3*.99,330e3*1.01],
                                     [330e3*.99,330e3*1.01],
                                     [100e3*.99,100e3*1.01]):
        parallel=1/(1/rb+1/rd)
        adc.append(parallel/(rt+parallel))
    assert max(adc)*4.3<1.0
    assert 4.3*(330e3*1.01)/(330e3*.99+330e3*1.01)<3.6
    off_adc=(2e-6+.1e-6)*100e3*1.01
    assert off_adc<.3
    main_off=4e-6*47e3*1.01
    assert main_off<.2
    # Piecewise ideal comparison pulses; these are stimuli, not a vendor XOR
    # model. Two independent RC corners check normal edge skew vs one mismatch
    # bit. The 2.2V trip allocation must be qualified on real U25 and rail.
    common='''
.control
set noaskquit
set wr_singlescale
set wr_vecnames
tran 20n 1.5m
'''
    skew=run_spice(args.ngspice,'tamper-skew',f'''OpenKeys active-loop skew RC - ideal stimulus only
Vdiff diff 0 PULSE(0 3.333 1m 1n 1n 1u 3m)
R66 diff filtered {4700*.99}
C62 filtered 0 {10e-9*.9}
{common}
meas tran skew_peak MAX v(filtered) FROM=1m TO=1.01m
wrdata tamper-skew-transient.txt v(filtered) v(diff)
quit
.endc
.end
''')
    opened=run_spice(args.ngspice,'tamper-open',f'''OpenKeys active-loop mismatch RC - ideal stimulus only
Vdiff diff 0 PULSE(0 2.55 1m 1n 1n {1/4096} 3m)
R66 diff filtered {4700*1.01}
C62 filtered 0 {10e-9*1.1}
Iinput filtered 0 5u
{common}
meas tran mismatch_cross WHEN v(filtered)=2.2 RISE=1
wrdata tamper-open-transient.txt v(filtered) v(diff)
quit
.endc
.end
''')
    def measured(log,name):
        match=re.search(r'(?m)^'+name+r'\s*=\s*([0-9.eE+-]+)',log)
        assert match, (name,'measurement missing')
        return float(match[1])
    peak=measured(skew,'skew_peak')
    crossing=measured(opened,'mismatch_cross')-.001
    assert 0<peak<.1 and 0<crossing<1/4096
    # A zero-run, or a physical bridge preserving the exact waveform, can
    # evade an open-to-ground comparison for this observation window. Never
    # turn a PRNG assumption into a guaranteed whole-system response time.
    def trips(word,inverted):
        filtered=-5e-6*(4700*1.01)
        for bit in range(8):
            high=bool(word & (1<<bit)) ^ inverted
            target=(2.55 if high else 0)-5e-6*(4700*1.01)
            filtered=target+(filtered-target)*math.exp(-(1/4096)/(4700*1.01*10e-9*1.1))
            if filtered>2.2:return True
        return False
    ground_trips=sum(trips(word,False) for word in range(256))
    high_trips=sum(trips(word,True) for word in range(256))
    assert ground_trips==high_trips==255
    patterns={'open_to_ground_nonzero_8bit_patterns':ground_trips,
              'stuck_high_not_all_ones_8bit_patterns':high_trips,
              'all_zero_pattern_no_mismatch':1,
              'matched_waveform_bridge_detected':False,
              'guaranteed_end_to_end_detection_latency':False}
    report={
        'schema':1,'revision':'0.5','status':'ALLOCATIONS_AND_PASSIVE_MODELS_ONLY',
        'netlist_sha256':hashlib.sha256(xml.read_bytes()).hexdigest(),
        'electrical_input_sha256':electrical_sha,'model_source_sha256':source_sha,
        'selected_parts':expected,
        'primary_references':[
            {'url':'https://www.st.com/resource/en/datasheet/stm32h573ri.pdf',
             'revision':'DS14121 Rev5','used':'VBAT1.2..3.6V; backup-SRAM-off RTC bounds; VBAT-capable pin map'},
            {'url':'https://www.ti.com/lit/ds/symlink/lm66100.pdf',
             'revision':'SLVSEZ8A','used':'pinout; CE=VOUT; 300nA IQ max; -250mV on-comparator corner; reverse leakage is nonzero'},
            {'url':'https://www.ti.com/lit/ds/symlink/sn74aup2g17.pdf',
             'revision':'SCES750A','used':'dual Schmitt; Ioff0.6uA per live input, II0.5uA; -40..85C'},
            {'url':'https://www.ti.com/lit/ds/symlink/sn74aup1g99.pdf',
             'revision':'SCES594C','used':'hard-strapped XOR truth table; fast conditioned inputs required'},
            {'url':'https://www.ti.com/lit/ds/symlink/sn74lvc1g14.pdf',
             'revision':'SCES218AA','used':'Schmitt RC receiver; threshold across actual rail requires qualification'},
            {'url':'https://www.ti.com/lit/ds/symlink/tmux1511.pdf',
             'revision':'SCDS390B','used':'RWB0012A; source/drain protected below1.5V; IPOFF max2uA; no control-only substitution'},
        ],
        'backup':{'vbat_operating_v':[1.2,3.6], 'backup_sram_enabled':False,
                  'primary_charge_allowed':False,'mcu_vbe_must_be_zero':True,
                  'rtc25c_max_a':1.2e-6,'rtc85c_max_a':3.65e-6,
                  'backup_voltage_temperature_monitor_required':True,
                  'sensor_buffer_bound_a':6.5e-6,
                  'extra_active_engine_current_allocation_a':10e-6,
                  'board_leakage_allocation_a':1e-6,
                  'path_headroom_allocation_v':.250,'corners':budget,
                  'capacity_and_active_current_qualified':False,
                  'guaranteed_service_life':False},
        'loop':{'return_low_allocation_v':return_low,'drive_low_allocation_v':drive_low,
                'powered_schmitt_3v_negative_threshold_min_v':.88,
                'rc_nominal_s':4700*10e-9,'assumed_closed_skew_s':1e-6,
                'intentional_return_rc_nominal_s':1000*100e-12,
                'ideal_filter_13_56mhz_attenuation_db':-10*math.log10(1+(2*math.pi*13.56e6*1000*.99*100e-12*.95)**2),
                'additional_loop_capacitance_allocation_f':300e-12,
                'skew_peak_v':peak,'assumed_trip_v':2.2,
                'one_mismatch_bit_s':1/4096,'passive_crossing_s':crossing,
                'stimulus_limits':patterns},
        'adc':{'on_nominal_ratio':100/530,'on_1percent_corners': [min(adc),max(adc)],
               'max_measured_pack_allocation_v':4.3,
               'powered_off_switch_bound_a':2e-6,
               'extra_off_leakage_allocation_a':.1e-6,
               'powered_off_adc_allocation_v':off_adc,
               'aggregate_main_off_feed_allocation_a':4e-6,
               'main_off_bleed_allocation_v':main_off,
               'reservoir_upstream_of_isolation':True,
               'accurate_soc_percentage_qualified':False},
        'unmodeled':['primary capacity/age/cold discharge and reverse charge acceptance',
                    'active TAMP/LSE startup and current; PRNG runs; backup switching',
                    'VDD/VBAT ramps, partial power, wet surfaces, off leakage and transient injection',
                    'actual gate thresholds/delays, logic skew, reset fanout, tamper-contact mechanics',
                    'matched injection, probe bridging, EM/voltage/clock faults and side channels'],
        'physical_qualification':False,'production_release':False}
    report_path.write_text(json.dumps(report,indent=2)+'\n')
    print('Backup/ADC allocations and two passive RC simulations passed; lifetime and physical security NOT qualified')


if __name__=='__main__':
    main()
