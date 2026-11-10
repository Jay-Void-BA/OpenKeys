#!/usr/bin/env python3
"""Derive an R04 sourcing worksheet, not an order or production release.

No component selection, price lookup, purchase, provisioning or flashing occurs.
The PCB source BOM remains authoritative; connector labels are disambiguated
and off-board parts are listed separately to avoid double purchasing.
"""
import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
BOM = 'hardware/r04/BOM.csv'
CPL = 'hardware/r04/exports/OpenKeys-SMD-position.csv'
EXPECTED_BOM = '961394d1949b5f86e42d99c3772350bf48aff0a0e6e43b2662c5082ae55435eb'
EXPECTED_PCB = '6c2a93a63f343cf0401eae8c92dc1da6f5db145604ee9b0e8b9bd08b76f4bde4'


def sha(name):
    return hashlib.sha256((ROOT / name).read_bytes()).hexdigest()


def encoded_csv(headers, rows):
    stream = io.StringIO(newline='')
    writer = csv.writer(stream, lineterminator='\n')
    writer.writerow(headers)
    writer.writerows(rows)
    return stream.getvalue().encode('utf-8')


def outputs():
    if sha(BOM) != EXPECTED_BOM or sha('hardware/r04/kicad/OpenKeys.kicad_pcb') != EXPECTED_PCB:
        raise ValueError('R04 source changed: review worksheet rules and manual before regenerating')
    bom = list(csv.DictReader((ROOT / BOM).read_text().splitlines()))
    cpl = list(csv.DictReader((ROOT / CPL).read_text().splitlines()))
    placed = {r['Ref'] for r in cpl}
    if len(bom) != 130 or len(cpl) != 126 or len(placed) != 126:
        raise ValueError('Unexpected R04 placement inventory')
    critical = {r['reference']: r for r in csv.DictReader(
        (ROOT / 'manufacturing/critical-parts.csv').read_text().splitlines())}
    overrides = {
        'J1': ('USB-C receptacle, exact HRO drawing and mating height', 'TYPE-C-31-M-12'),
        'J2': ('JST PH 2-pin horizontal BOARD connector; BAT1 is separate', ''),
        'J5': ('Hirose BOARD FPC connector; FP1 module is separate', 'FH12-12S-0.5SH(55)'),
        'J6': ('JST SH 2-pin BOARD connector; NTC and cable are separate', ''),
        'SW1': ('Mechanical power slide switch', 'JS102011SAQN'),
        **{f'SW{i}': ('Tact switch; SW6 is the lid contact', 'TL3342F160QG') for i in range(2, 7)},
        'L1': ('Power inductor; confirm Isat/current/temperature', 'SWPA3015S1R5MT'),
    }
    rows = []
    for r in bom:
        ref = r['ref']
        dnp = r['dnp'].lower() == 'true'
        integral = ref == 'J4'
        quantity = 0 if dnp or integral else 1
        stage = 'DNP_PRODUCTION_FIXTURE_ONLY' if dnp else 'PCB_INTEGRAL_NO_PURCHASE' if integral else 'SMT' if ref in placed else 'POST_REFLOW_THT'
        item, candidate = overrides.get(ref, (r['value'], critical.get(ref, {}).get('part_number', '')))
        status = 'NO_PURCHASE' if not quantity else 'CANDIDATE_NEEDS_DATASHEET_AND_SUPPLIER_APPROVAL' if candidate else 'FULL_ORDERABLE_MPN_AND_RATINGS_REQUIRED'
        notes = r['note']
        if ref == 'F1':
            notes += ' Exact resettable-fuse MPN and current/temperature/I2t coordination remain unresolved.'
        if ref in ('J2', 'J6'):
            notes += ' Source value labels the connected assembly, not the board connector order code.'
        if ref == 'J5':
            notes += ' Buy one connector here and one 100026230 off-board module; not two modules.'
        rows.append([ref, r['value'], item, r['footprint'], stage, quantity, quantity * 3,
                     candidate, status, '', '', '', '', '', '', '', notes.strip()])
    headers = ['ref', 'source_value', 'purchase_item', 'footprint', 'assembly_stage',
               'qty_per_unit', 'qty_for_3_final_units', 'design_candidate_mpn', 'sourcing_status',
               'approved_manufacturer', 'approved_full_mpn', 'supplier_sku', 'datasheet_url',
               'quoted_unit_price_cny', 'actual_order_qty_including_loss', 'lot_traceability', 'notes']
    external = [
        ['FP1', 'FPC2534AM flex module', '100026230', 1, 3, 'Off-board; J5 connector is already in board worksheet; no AP/LGA substitution'],
        ['BAT1', 'Protected 1S 3.7V 500mAh pack', 'Adafruit 1578', 1, 3, 'Pack includes battery lead; no bare cell; measure J2 mating polarity'],
        ['NTC1', 'Actual on-cell insulated thermistor', 'Semitec 103AT-2', 1, 3, '10k/B3435; not a fixed resistor; no soldering to pouch'],
        ['NTC_CABLE', 'Factory-crimped JST SH mating cable for J6', 'Exact mating plug/crimps to be approved', 1, 3, 'Two wires from real NTC; verify J6.1 BAT_NTC and J6.2 GND'],
        ['CASE_BASE', 'Nonmetal case base', 'OpenKeys-base.stl', 1, 3, 'Engineering material/tolerances must be qualified; no metal near NFC loop'],
        ['CASE_LID', 'Case lid', 'OpenKeys-lid.stl', 1, 3, 'No proven waterproof or active-mesh claim'],
        ['FP_RETAINER', 'Fingerprint retainer', 'OpenKeys-retainer.stl', 1, 3, 'Confirm flex exit, clearance and sensor support'],
        ['PLUNGER_SET', 'One set contains FOUR button plungers', 'OpenKeys-plungers.stl', 1, 3, 'Order 3 sets = 12 individual plungers; test travel and retention'],
        ['WINDOW', 'Hard-coated transparent PC window 49x17.6x0.8mm', 'Cut to reviewed drawing', 1, 3, 'Final fit/adhesive qualification required'],
        ['M2_MAIN', 'M2 main fasteners', 'Length/head/thread after physical fit review', 4, 12, 'Not a universal screw length; no battery or antenna intrusion'],
        ['M1_6_RETAINER', 'M1.6 fingerprint-retainer fasteners', 'Length/head/thread after physical fit review', 2, 6, 'No overtightening of sensor/flex'],
        ['M2_INSULATING', 'Insulating M2 lid-contact adjuster', 'Fit to reviewed drawing', 1, 3, 'Set SW6 preload in assembly jig; not metal'],
        ['INSULATION_SET', 'Insulation pads/thermal tape/soft restraint set', 'Assembler-approved materials and geometry', 1, 3, 'No pressure on pouch, no conductive foil/metal in RF keepout'],
    ]
    payloads = {
        'manufacturing/procurement-r04-board.csv': encoded_csv(headers, rows),
        'manufacturing/procurement-r04-external.csv': encoded_csv(
            ['item', 'description', 'candidate_or_drawing', 'qty_per_unit', 'qty_for_3_final_units', 'notes'], external),
    }
    report = {
        'schema': 1, 'purpose': 'ENGINEERING_SOURCING_WORKSHEET_ONLY',
        'production_order_authorized': False, 'prices_or_stock_confirmed': False,
        'quantities_exclude_evt_extra_boards_and_assembler_loss': True,
        'bom_positions': len(bom), 'smt_positions': len(cpl),
        'smt_top': sum(r['Side'] == 'top' for r in cpl),
        'smt_bottom': sum(r['Side'] == 'bottom' for r in cpl),
        'tht_positions': ['J2', 'U8'], 'production_dnp': ['J3'], 'integral_copper': ['J4'],
        'board_purchased_positions_per_unit': sum(r[5] for r in rows),
        'board_outline_body_mm': [68, 92], 'top_tab_mm': [12, 2.5],
        'maximum_outline_centerline_envelope_mm': [68, 94.5],
        'minimum_track_clearance_mm': 0.15, 'via_copper_drill_mm': [0.6, 0.3],
        'sources_sha256': {name: sha(name) for name in (BOM, CPL,
            'hardware/r04/kicad/OpenKeys.kicad_pcb', 'hardware/r04/pinout.csv',
            'hardware/r04/exports/OpenKeys-netlist.xml', 'manufacturing/critical-parts.csv')},
        'worksheet_sha256': {name: hashlib.sha256(data).hexdigest() for name, data in payloads.items()},
        'limits': 'Full orderable MPNs, ratings, sourcing, process losses, DFM, security integration and physical qualification still require review.'
    }
    payloads['manufacturing/procurement-r04-manifest.json'] = (json.dumps(report, indent=2) + '\n').encode()
    return payloads


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    try:
        for name, data in outputs().items():
            path = ROOT / name
            if args.check:
                if not path.exists() or path.read_bytes() != data:
                    raise ValueError(f'Stale/missing sourcing worksheet: {name}')
            else:
                path.write_bytes(data)
    except (OSError, ValueError, KeyError) as error:
        print(str(error), file=sys.stderr)
        return 1
    print('R04 worksheet: 130 positions; 126 SMT, 2 THT, 1 DNP, 1 integral coil; no order or production release')
    return 0


if __name__ == '__main__':
    sys.exit(main())
