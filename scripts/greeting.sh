#!/bin/bash
# Draw a display's greeting from the Audiophonics mark: the mark above, the
# product's name beneath it in a quieter grey, as a binary PGM of the
# panel's size. The panel shows eight-bit grey on its own sixteen levels,
# so the mark's soft edges survive.
#
#   scripts/greeting.sh "EVO SABRE" plugins/audiophonics_evosabre/assets/logo.pgm
#
# Needs ImageMagick 7 and the DejaVu Sans face. The result is committed:
# run this when the mark or a product's name changes.
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
NAME=${1:?the name of the product, such as EVO SABRE}
OUT=${2:?the PGM to write}
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

magick "$ROOT/artwork/audiophonics.png" -colorspace gray -filter Lanczos -resize 214x \
  -background black -gravity center -extent 256x40 "$WORK/mark.png"
magick -size 256x24 xc:black -font DejaVu-Sans -pointsize 13 -kerning 5 -fill 'gray(60%)' \
  -gravity center -annotate +2+0 "$NAME" "$WORK/name.png"
magick "$WORK/mark.png" "$WORK/name.png" -append -colorspace gray -depth 8 "$OUT"
magick identify "$OUT"
