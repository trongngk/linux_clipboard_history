#!/usr/bin/env bash
# Full, idempotent setup for cliphist. Safe to re-run.
# Handles both X11 (xdotool/xclip) and Wayland (ydotool/wl-clipboard).
# For the fast rebuild loop after this has run once, use ./update.sh instead.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="$HOME/.local/bin"
BIN="$BIN_DIR/cliphist"
TARGET="$REPO/target/release/cliphist"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/cliphist-lite"

say() { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
have() { command -v "$1" >/dev/null 2>&1; }

# --- session detection ---------------------------------------------------
if [ -n "${WAYLAND_DISPLAY:-}" ] || [ "${XDG_SESSION_TYPE:-}" = "wayland" ]; then
  SESSION=wayland
else
  SESSION=x11
fi
say "Session: $SESSION   Desktop: ${XDG_CURRENT_DESKTOP:-unknown}"

# --- dependencies --------------------------------------------------------
missing=()
if [ "$SESSION" = wayland ]; then
  have ydotool  || missing+=(ydotool)
  have wl-copy  || missing+=(wl-clipboard)
else
  have xdotool  || missing+=(xdotool)
  have xclip    || missing+=(xclip)
fi
if [ "${#missing[@]}" -gt 0 ]; then
  say "Installing: ${missing[*]}  (needs sudo)"
  sudo apt update -qq
  sudo apt install -y "${missing[@]}"
else
  say "Runtime tools already installed"
fi

# --- Wayland: /dev/uinput access for ydotool -----------------------------
if [ "$SESSION" = wayland ]; then
  if ! id -nG | tr ' ' '\n' | grep -qx input; then
    say "Adding $USER to 'input' group and installing uinput udev rule (needs sudo)"
    sudo usermod -aG input "$USER"
    echo 'KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"' \
      | sudo tee /etc/udev/rules.d/80-uinput.rules >/dev/null
    sudo udevadm control --reload-rules && sudo udevadm trigger
    NEED_RELOGIN=1
  else
    say "uinput access already granted"
  fi

  # Start ydotoold (kernel-level virtual input daemon)
  if systemctl --user list-unit-files 2>/dev/null | grep -q '^ydotoold\.service'; then
    systemctl --user enable --now ydotoold || true
  elif ! pgrep -x ydotoold >/dev/null; then
    say "Starting ydotoold in the background"
    nohup ydotoold >"$DATA_DIR/ydotoold.log" 2>&1 &
  fi
fi

# --- build ---------------------------------------------------------------
say "Building (release)"
( cd "$REPO" && cargo build --release )

# --- install binary as a symlink (so updates need no copy) ---------------
mkdir -p "$BIN_DIR" "$DATA_DIR"
ln -sfn "$TARGET" "$BIN"
say "Linked $BIN -> $TARGET"

# --- autostart the daemon at login ---------------------------------------
AUTOSTART="$HOME/.config/autostart/cliphist-lite.desktop"
mkdir -p "$(dirname "$AUTOSTART")"
cat >"$AUTOSTART" <<EOF
[Desktop Entry]
Type=Application
Name=Clipboard History Daemon
Exec=$BIN daemon
X-GNOME-Autostart-enabled=true
NoDisplay=true
EOF
say "Autostart written: $AUTOSTART"

# --- Super+V hotkey ------------------------------------------------------
case "${XDG_CURRENT_DESKTOP:-}" in
  *Cinnamon*|*CINNAMON*)
    "$BIN" install --hotkey '<Super>v' >/dev/null || true
    say "Cinnamon hotkey bound via cliphist install"
    ;;
  *GNOME*|*ubuntu*|*Unity*)
    P=/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/cliphist/
    KB=org.gnome.settings-daemon.plugins.media-keys.custom-keybinding
    gsettings set "$KB:$P" name 'cliphist'
    gsettings set "$KB:$P" command "$BIN pick --paste"
    gsettings set "$KB:$P" binding '<Super>v'
    # Ensure our path is in the custom-keybindings list.
    LIST=$(gsettings get org.gnome.settings-daemon.plugins.media-keys custom-keybindings)
    if [[ "$LIST" != *"$P"* ]]; then
      if [ "$LIST" = "@as []" ] || [ "$LIST" = "[]" ]; then
        gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings "['$P']"
      else
        gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings \
          "${LIST%]}, '$P']"
      fi
    fi
    say "GNOME hotkey Super+V bound"
    ;;
  *)
    say "Unknown desktop - bind Super+V to '$BIN pick --paste' in Settings > Keyboard"
    ;;
esac

# --- (re)start the daemon ------------------------------------------------
pkill -f "cliphist daemon" 2>/dev/null || true
sleep 0.3
nohup "$BIN" daemon >"$DATA_DIR/daemon.log" 2>&1 &
say "Daemon started (log: $DATA_DIR/daemon.log)"

echo
say "Done. Press Super+V to open the panel."
if [ "${NEED_RELOGIN:-0}" = 1 ]; then
  printf '\033[1;33m!!  Log out and back in once\033[0m so the new "input" group takes effect,\n'
  printf '    otherwise ydotool auto-paste will fail with a permission error.\n'
fi
