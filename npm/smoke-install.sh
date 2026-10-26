#!/usr/bin/env bash
# Installs the packed tarballs (node npm/build.mjs --pack) into a fresh project and runs the launcher, the path helper,
# the JS API and the headless smoke test against the installed packages.
#
#   npm/smoke-install.sh [npm/dist]
set -euo pipefail

exec node "$(dirname "$0")/smoke-install.mjs" "$@"
