#!/bin/sh
# SPDX-License-Identifier: CERN-OHL-S-2.0
# Verify the saved routed board. Never rebuild/reroute it implicitly.
set -eu
cd "$(dirname "$0")/../../.."
openkeys_r05_state=$(mktemp -d "${TMPDIR:-/tmp}/openkeys-r05.XXXXXX")
trap 'rm -rf "$openkeys_r05_state"' EXIT HUP INT TERM
export XDG_CONFIG_HOME="$openkeys_r05_state/config"
export XDG_CACHE_HOME="$openkeys_r05_state/cache"
export XDG_DATA_HOME="$openkeys_r05_state/data"
mkdir -p "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$XDG_DATA_HOME"
openkeys_r05=hardware/r05
openkeys_board="$openkeys_r05/kicad/OpenKeys.kicad_pcb"
openkeys_schematic="$openkeys_r05/kicad/OpenKeys.kicad_sch"
kicad-cli sch erc --format json --severity-all --output "$openkeys_r05/review/erc.json" "$openkeys_schematic"
kicad-cli sch export netlist --format kicadxml --output "$openkeys_r05/exports/OpenKeys-netlist.xml" "$openkeys_schematic"
python3 "$openkeys_r05/tools/check_netlist.py"
kicad-cli pcb drc --format json --severity-all --output "$openkeys_r05/review/routed-drc.json" "$openkeys_board"
/usr/bin/python3 "$openkeys_r05/tools/check_board.py"
/usr/bin/python3 "$openkeys_r05/tools/ground_report.py"
python3 "$openkeys_r05/tools/calculate_protection.py"
python3 "$openkeys_r05/tools/calculate_lcd_window.py"
python3 "$openkeys_r05/tools/display_model.py"
kicad-cli sch export pdf --output "$openkeys_r05/exports/OpenKeys-schematic.pdf" "$openkeys_schematic"
kicad-cli sch export svg --output "$openkeys_r05/exports/schematic/" "$openkeys_schematic"
python3 - <<'PY'
from pathlib import Path
for path in Path('hardware/r05/exports/schematic').glob('*.svg'):
    path.write_text('\n'.join(line.rstrip() for line in path.read_text().splitlines()).rstrip()+'\n')
PY
/usr/bin/python3 "$openkeys_r05/tools/drawings.py"
python3 - <<'PY_DRAWING'
import cairosvg
from pathlib import Path
path=Path('hardware/r05/exports/OpenKeys-dimensions.svg')
cairosvg.svg2pdf(url=str(path),write_to=str(path.with_suffix('.pdf')))
cairosvg.svg2png(url=str(path),write_to=str(path.with_suffix('.png')))
PY_DRAWING
mkdir -p "$openkeys_r05/exports/fabrication"
kicad-cli pcb export gerbers --layers F.Cu,In1.Cu,In2.Cu,B.Cu,F.Mask,B.Mask,F.Silkscreen,B.Silkscreen,Edge.Cuts --use-drill-file-origin --output "$openkeys_r05/exports/fabrication/" "$openkeys_board"
kicad-cli pcb export drill --format excellon --drill-origin plot --excellon-units mm --excellon-separate-th --generate-map --map-format pdf --output "$openkeys_r05/exports/fabrication/" "$openkeys_board"
kicad-cli pcb export pos --format csv --units mm --side both --smd-only --exclude-dnp --use-drill-file-origin --output "$openkeys_r05/exports/OpenKeys-SMD-position.csv" "$openkeys_board"
kicad-cli pcb export svg --mode-single --fit-page-to-board --exclude-drawing-sheet --layers F.Cu,F.Fab,Edge.Cuts --output "$openkeys_r05/exports/OpenKeys-PCB-front.svg" "$openkeys_board"
kicad-cli pcb export svg --mode-single --fit-page-to-board --exclude-drawing-sheet --mirror --layers B.Cu,B.Fab,Edge.Cuts --output "$openkeys_r05/exports/OpenKeys-PCB-back.svg" "$openkeys_board"
kicad-cli pcb export step --board-only --cut-vias-in-body --user-origin 100x80mm --force --output "$openkeys_r05/exports/OpenKeys-PCB.step" "$openkeys_board"
for openkeys_part in base lid plungers retainer; do
    openscad --export-format binstl -o "$openkeys_r05/exports/OpenKeys-${openkeys_part}.stl" -D "part=\"${openkeys_part}\"" "$openkeys_r05/mechanical/enclosure.scad"
done
python3 - <<'PY_TEXT'
from pathlib import Path
for path in Path('hardware/r05/exports').rglob('*'):
    if path.is_file() and path.suffix in ('.svg', '.step', '.csv'):
        path.write_text('\n'.join(line.rstrip() for line in path.read_text().splitlines()).rstrip()+'\n')
PY_TEXT
# Passive simulations and Blender rendering are separate explicit commands.
python3 "$openkeys_r05/tools/manifest.py"
