#!/bin/sh
# SPDX-License-Identifier: CERN-OHL-S-2.0
set -eu
cd "$(dirname "$0")/../.."
openkeys_cad_state=$(mktemp -d "${TMPDIR:-/tmp}/openkeys-cad.XXXXXX")
trap 'rm -rf "$openkeys_cad_state"' EXIT HUP INT TERM
export XDG_CONFIG_HOME="$openkeys_cad_state/config"
export XDG_CACHE_HOME="$openkeys_cad_state/cache"
export XDG_DATA_HOME="$openkeys_cad_state/data"
mkdir -p "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$XDG_DATA_HOME" hardware/exports/schematic hardware/review
python3 hardware/tools/schematic.py
/usr/bin/python3 hardware/tools/mechanical.py
kicad-cli sch erc --format json --severity-all --output hardware/review/erc.json hardware/kicad/OpenKeys.kicad_sch
kicad-cli sch export netlist --format kicadxml --output hardware/exports/OpenKeys-netlist.xml hardware/kicad/OpenKeys.kicad_sch
python3 hardware/tools/check_netlist.py
kicad-cli sch export pdf --output hardware/exports/OpenKeys-schematic.pdf hardware/kicad/OpenKeys.kicad_sch
kicad-cli sch export svg --output hardware/exports/schematic/ hardware/kicad/OpenKeys.kicad_sch
openscad --export-format binstl -o hardware/exports/OpenKeys-base.stl -D 'part="base"' hardware/mechanical/OpenKeys.scad
openscad --export-format binstl -o hardware/exports/OpenKeys-lid.stl -D 'part="lid"' hardware/mechanical/OpenKeys.scad
openscad --export-format binstl -o hardware/exports/OpenKeys-plungers.stl -D 'part="plungers"' hardware/mechanical/OpenKeys.scad
