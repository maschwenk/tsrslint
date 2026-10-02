#!/bin/bash
# Times the tsgolint segment of an oxlint run: appends "<impl> <start> <end>" (epoch seconds) to $SEG_LOG.
s=$(python3 -c 'import time;print(time.time())')
"$REAL_TSGOLINT" "$@"
ec=$?
e=$(python3 -c 'import time;print(time.time())')
echo "$SEG_IMPL $s $e" >> "$SEG_LOG"
exit $ec

