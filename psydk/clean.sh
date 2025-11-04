#!/usr/bin/env bash
set -euo pipefail

root="${1:-.}"

find "$root" -type f -name '*.lock' -print -delete