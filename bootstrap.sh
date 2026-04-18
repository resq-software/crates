#!/usr/bin/env sh
# Copyright 2026 ResQ
# SPDX-License-Identifier: Apache-2.0
#
# Canonical onboarding — delegates to resq-software/dev.
# See https://github.com/resq-software/dev for the full installer.
set -eu
export REPO=crates
INSTALLER="$(curl -fsSL https://raw.githubusercontent.com/resq-software/dev/main/install.sh)" || {
  echo "bootstrap: failed to download installer" >&2
  exit 1
}
[ -n "$INSTALLER" ] || { echo "bootstrap: empty installer payload" >&2; exit 1; }
exec sh -c "$INSTALLER" -- "$@"
