#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""Remove unused single-layer escape vias, never an ERC/DRC exclusion.

Run AFTER routing and a fresh DRC. Refuses disconnected nets and every other
violation. Then run DRC/parity again on the physically edited saved board.
"""
import json
from pathlib import Path
import pcbnew as p

R=Path(__file__).resolve().parents[1]
report=json.loads((R/'review/routed-drc.json').read_text())
assert not report['unconnected_items'] and not report['schematic_parity']
assert all(z['type']=='via_dangling' and len(z['items'])==1 for z in report['violations'])
ids={z['items'][0]['uuid'] for z in report['violations']}
if ids:
    path=R/'kicad/OpenKeys.kicad_pcb';b=p.LoadBoard(str(path))
    found=[z for z in b.GetTracks() if z.GetClass()=='PCB_VIA' and z.m_Uuid.AsString() in ids]
    assert len(found)==len(ids),'Stale DRC report'
    for z in found:b.RemoveNative(z)
    assert p.ZONE_FILLER(b).Fill(b.Zones())
    p.SaveBoard(str(path),b)
print(f'Removed {len(ids)} unused vias; rerun full DRC and PCB/netlist parity')
