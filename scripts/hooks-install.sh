#!/usr/bin/env bash
# Enables the repository hooks from .githooks/ (pre-commit: cargo xtask docs check).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
git config core.hooksPath .githooks
printf 'hooks enabled: core.hooksPath=.githooks\n'
