#!/usr/bin/env bash
# Records one py-spy profile of a Python script.
# usage: py_spy_record.sh PY_SPY FORMAT OUTPUT PYTHON SCRIPT [ARGS...]
# Fails when the script exits non-zero, when py-spy fails, or when OUTPUT is missing or empty.
# The one failure py-spy 0.4 reports after it has written its output, "No child process" when the
# script ends while it is still sampling, is tolerated; py-spy itself exits 0 whatever the
# script's status, so the script runs under a wrapper that records it.
set -uo pipefail

if [ "$#" -lt 5 ]; then
  echo "usage: $0 PY_SPY FORMAT OUTPUT PYTHON SCRIPT [ARGS...]" >&2
  exit 2
fi
py_spy=$1 format=$2 output=$3
shift 3

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
rm -f "$output"

# The inner script is single-quoted on purpose: sh expands it, not this shell.
# shellcheck disable=SC2016
"$py_spy" record --subprocesses --format "$format" -o "$output" -- \
  sh -c '"$@"; status=$?; echo "$status" > "$0"; exit "$status"' "$work/status" "$@" 2> "$work/stderr"
spy_status=$?
cat "$work/stderr" >&2

script_status=$(cat "$work/status" 2>/dev/null || echo missing)
if [ "$script_status" != 0 ]; then
  echo "profile-py: the script ($*) exited with status $script_status; no profile recorded" >&2
  rm -f "$output"
  exit 1
fi
if [ "$spy_status" != 0 ]; then
  errors=$(grep '^Error' "$work/stderr" | grep -vc '^Error: No child process (os error 10)$')
  if [ "$errors" != 0 ]; then
    echo "profile-py: py-spy exited with status $spy_status while recording $output" >&2
    exit 1
  fi
fi
if [ ! -s "$output" ]; then
  echo "profile-py: py-spy wrote no $output" >&2
  exit 1
fi
