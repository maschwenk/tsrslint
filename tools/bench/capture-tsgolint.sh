#!/bin/bash
# Records oxlint's tsgolint invocation (args + stdin payload) then runs the real binary.
OUT=${CAPTURE_DIR:-/tmp}
echo "$@" > "$OUT/args.txt"
tee "$OUT/payload.json" | "${REAL_TSGOLINT}" "$@"

