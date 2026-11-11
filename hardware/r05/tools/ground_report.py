#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""Inventory the saved PCB ground pads; do not infer an RF return path."""
import hashlib
import json
from pathlib import Path

import pcbnew as p

root = Path(__file__).resolve().parents[1]
path = root / 'kicad/OpenKeys.kicad_pcb'
board = p.LoadBoard(str(path))
vias = [item for item in board.GetTracks()
        if item.GetClass() == 'PCB_VIA' and item.GetNetname() == 'GND']
if not vias:
    raise SystemExit('No saved GND vias')
connections = []
for footprint in sorted(board.GetFootprints(), key=lambda item: item.GetReference()):
    for pad in footprint.Pads():
        if pad.GetNetname() != 'GND' or pad.GetAttribute() != p.PAD_ATTRIB_SMD:
            continue
        via = min(vias, key=lambda item: (item.GetPosition() - pad.GetPosition()).EuclideanNorm())
        connections.append({
            'ref': footprint.GetReference(), 'pad': pad.GetNumber(),
            'pad_mm': [p.ToMM(pad.GetPosition().x) - 100, p.ToMM(pad.GetPosition().y) - 80],
            'nearest_via_mm': [p.ToMM(via.GetPosition().x) - 100, p.ToMM(via.GetPosition().y) - 80],
            'straight_distance_mm': p.ToMM((via.GetPosition() - pad.GetPosition()).EuclideanNorm()),
        })
report = {
    'schema': 2, 'revision': '0.5',
    'board_sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
    'smd_ground_pads': len(connections), 'saved_ground_vias': len(vias),
    'meaning': 'Actual saved pad/via positions. Nearest via is geometric, not a routed '
               'length, a proof of direct attachment, impedance or physical qualification. '
               'Use the independent KiCad DRC for logical connectivity.',
    'connections': connections,
}
(root / 'review/ground-fanout.json').write_text(json.dumps(report, indent=2) + '\n')
print(f'Inventoried {len(connections)} SMD ground pads and {len(vias)} saved ground vias')
