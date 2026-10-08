#!/bin/bash
# Alternating stock-tsgolint vs tsrslint timing (median-of-pairs methodology used in docs/evaluation.md).
# Usage: bench.sh <pairs> <direct|e2e> <out.tsv>
# Env: STOCK_TSGOLINT (stock binary), RS (tsrslint binary), MONOLITH (the monolith's checkout with deps built and
#      .oxlintrc.tsrslint.json), PAYLOAD (captured oxlint payload filtered to the slice rules), BENCH (this dir),
#      TIMEFILES (scratch dir for /usr/bin/time output).
set -u
: "${STOCK_TSGOLINT:?}" "${RS:?}" "${MONOLITH:?}" "${PAYLOAD:?}" "${BENCH:?}" "${TIMEFILES:?}"
PAIRS=$1; MODE=$2; OUT=$3
mkdir -p "$TIMEFILES"
cd "$MONOLITH"
echo -e "mode\tpair\timpl\twall_s\tuser_s\tsys_s\tmaxrss_gb\tload1\texit" >> "$OUT"
run() { # impl binary pair
  local impl=$1 bin=$2 pair=$3
  local load=$(sysctl -n vm.loadavg | awk '{print $2}')
  local tf=$TIMEFILES/$MODE-$pair-$impl-$(date +%s).txt
  if [ "$MODE" = direct ]; then
    /usr/bin/time -l "$bin" headless < "$PAYLOAD" > /dev/null 2> "$tf"; ec=$?
  else
    SEG_LOG=$OUT.seg SEG_IMPL=$impl-$pair REAL_TSGOLINT=$bin OXLINT_TSGOLINT_PATH=$BENCH/seg-wrapper.sh NODE_OPTIONS=--max_old_space_size=14000 \
      /usr/bin/time -l ../../node_modules/.bin/oxlint -c .oxlintrc.tsrslint.json --type-aware --deny-warnings > /dev/null 2> "$tf"; ec=$?
  fi
  local wall=$(grep ' real ' "$tf" | awk '{print $1}') user=$(grep ' real ' "$tf" | awk '{print $3}') sys=$(grep ' real ' "$tf" | awk '{print $5}')
  local rss=$(grep 'maximum resident' "$tf" | awk '{printf "%.2f", $1/1073741824}')
  echo -e "$MODE\t$pair\t$impl\t$wall\t$user\t$sys\t$rss\t$load\t$ec" >> "$OUT"
}
for i in $(seq 1 "$PAIRS"); do
  if [ $((i % 2)) -eq 1 ]; then run stock "$STOCK_TSGOLINT" "$i"; run tsrslint "$RS" "$i"; else run tsrslint "$RS" "$i"; run stock "$STOCK_TSGOLINT" "$i"; fi
done
echo DONE >> "$OUT"
