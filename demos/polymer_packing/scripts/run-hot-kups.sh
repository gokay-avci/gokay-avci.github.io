#!/usr/bin/env sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
default_python="$project_root/../../kups-diff-polymer-demo/.venv/bin/python"
kups_python=${EVOKUPS_PYTHON:-$default_python}

if [ ! -x "$kups_python" ]; then
  echo "No kUPS Python environment found at: $kups_python" >&2
  echo "Set EVOKUPS_PYTHON to the Python executable containing jax and kups." >&2
  exit 1
fi

export PYTHONPATH="$project_root/python/kups_backend/src${PYTHONPATH:+:$PYTHONPATH}"
export EVOKUPS_KERNEL=${EVOKUPS_KERNEL:-kups}

exec "$kups_python" -m kups_backend.stdlib_service
