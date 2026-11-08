#!/bin/sh
# SPDX-License-Identifier: CERN-OHL-S-2.0
set -eu
cd "$(dirname "$0")/../../.."
openkeys_r03_state=$(mktemp -d "${TMPDIR:-/tmp}/openkeys-r03.XXXXXX")
trap 'rm -rf "$openkeys_r03_state"' EXIT HUP INT TERM
export XDG_CONFIG_HOME="$openkeys_r03_state/config"
export XDG_CACHE_HOME="$openkeys_r03_state/cache"
export XDG_DATA_HOME="$openkeys_r03_state/data"
mkdir -p "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$XDG_DATA_HOME"
/usr/bin/python3 hardware/r03/tools/footprints.py
python3 hardware/r03/tools/generate.py
/usr/bin/python3 hardware/r03/tools/mechanical.py
kicad-cli sch erc --format json --severity-all --output hardware/r03/review/erc.json hardware/r03/kicad/OpenKeys.kicad_sch
kicad-cli sch export netlist --format kicadxml --output hardware/r03/exports/OpenKeys-netlist.xml hardware/r03/kicad/OpenKeys.kicad_sch
python3 hardware/r03/tools/check_netlist.py
kicad-cli sch export pdf --output hardware/r03/exports/OpenKeys-schematic.pdf hardware/r03/kicad/OpenKeys.kicad_sch
kicad-cli sch export svg --output hardware/r03/exports/schematic/ hardware/r03/kicad/OpenKeys.kicad_sch
python3 - <<'PY_NORMALIZE'
from pathlib import Path
for path in Path('hardware/r03/exports/schematic').glob('*.svg'):
    path.write_text('\n'.join(line.rstrip() for line in path.read_text().splitlines()).rstrip()+'\n')
PY_NORMALIZE
openscad --export-format binstl -o hardware/r03/exports/OpenKeys-base.stl -D 'part="base"' hardware/r03/mechanical/OpenKeys.scad
openscad --export-format binstl -o hardware/r03/exports/OpenKeys-lid.stl -D 'part="lid"' hardware/r03/mechanical/OpenKeys.scad
openscad --export-format binstl -o hardware/r03/exports/OpenKeys-plungers.stl -D 'part="plungers"' hardware/r03/mechanical/OpenKeys.scad
