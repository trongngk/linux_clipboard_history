#!/usr/bin/env bash
# Fast rebuild loop: recompile and restart the daemon. No copy, no sudo.
# Relies on install.sh having symlinked ~/.local/bin/cliphist -> target/release.
# Pass --debug for a much faster (unoptimized) build while iterating.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/cliphist-lite"
BIN="$HOME/.local/bin/cliphist"

if [ "${1:-}" = "--debug" ]; then
  ( cd "$REPO" && cargo build )
  ln -sfn "$REPO/target/debug/cliphist" "$BIN"
else
  ( cd "$REPO" && cargo build --release )
  ln -sfn "$REPO/target/release/cliphist" "$BIN"
fi

pkill -f "cliphist daemon" 2>/dev/null || true
sleep 0.3
mkdir -p "$DATA_DIR"
nohup "$BIN" daemon >"$DATA_DIR/daemon.log" 2>&1 &
printf '\033[1;36m==>\033[0m rebuilt and daemon restarted. Super+V ready.\n'
