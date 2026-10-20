#!/usr/bin/env bash
# Runs a command with project env, then guarantees nothing it started is left running.
#   - CARGO_BUILD_JOBS=2 keeps the 8 GB dev machine out of swap.
#   - .env.local (git-ignored, written by setup-android.sh) supplies ANDROID_HOME, JAVA_HOME, ...
#   - On exit, Ctrl-C or SIGTERM the whole descendant tree is killed (vite, cargo, gradle...).
set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

export CARGO_BUILD_JOBS=2
if [ -f .env.local ]; then
  set -a
  # shellcheck disable=SC1091
  . ./.env.local
  set +a
fi

# Recursively kill a process and everything under it, children first.
kill_tree() {
  local pid=$1 child
  for child in $(pgrep -P "$pid" 2>/dev/null); do
    kill_tree "$child"
  done
  kill -TERM "$pid" 2>/dev/null || true
}

child=""
cleanup() {
  trap - EXIT INT TERM
  if [ -n "$child" ] && kill -0 "$child" 2>/dev/null; then
    kill_tree "$child"
    sleep 1
    kill -KILL "$child" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

# <&0 keeps the terminal attached (async commands otherwise get /dev/null), which
# interactive prompts such as Android device selection need.
"$@" <&0 &
child=$!
wait "$child"
status=$?
exit "$status"
