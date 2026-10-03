#!/usr/bin/env bash
# Installs the packed tarballs (node npm/build.mjs --pack) into a fresh project and runs the launcher, the path helper,
# the JS API and the headless smoke test against the installed packages.
#
#   npm/smoke-install.sh [npm/dist]
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
dist=$(cd "${1:-$repo/npm/dist}" && pwd)
dir=$(mktemp -d)
cd "$dir"
npm init -y > /dev/null
npm install --no-audit --no-fund "$dist"/*.tgz

npx --no-install tsrslint --version
bin=$(npx --no-install tsrslint-path)
echo "binary: $bin"
test -x "$bin"
api=$(node -p 'require("@maschwenk/tsrslint").binaryPath()')
[ "$api" = "$bin" ] || { echo "binaryPath() returned $api, tsrslint-path printed $bin"; exit 1; }
node -e 'const m = require("@maschwenk/tsrslint"); if (!m.version || !m.tsrsCommit) process.exit(1); console.log(m.version, m.tsrsCommit)'

# Through the Node launcher (args and stdio passed through) and the native binary directly, as OXLINT_TSGOLINT_PATH uses it.
node "$repo/npm/smoke.mjs" "$dir/node_modules/.bin/tsrslint"
node "$repo/npm/smoke.mjs" "$bin"
