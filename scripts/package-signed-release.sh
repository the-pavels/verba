#!/bin/bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

export VERBA_SIGNING_MODE=developer-id
exec "${repo_root}/scripts/package-release.sh" "$@"
