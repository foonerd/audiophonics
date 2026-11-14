#!/bin/bash
# Draw the Audiophonics mark for a colour display's start and leaving
# screens: the mark alone, light on dark, at the width given, as a binary
# PGM. The display lays white on by the mark's grey, so its soft edges
# survive, and puts its own word under it.
#
#   scripts/mark.sh 272 plugins/audiophonics_raspdacminilcd/assets/mark.pgm
#
# Needs ImageMagick 7. The result is committed: run this when the mark
# changes.
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
WIDTH=${1:?the width of the mark in pixels, such as 272}
OUT=${2:?the PGM to write}

magick "$ROOT/artwork/audiophonics.png" -colorspace gray -filter Lanczos -resize "${WIDTH}x" \
  -depth 8 "$OUT"
magick identify "$OUT"
