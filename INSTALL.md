# Installing cliphist

Clipboard history for Linux, used like `Win+V` on Windows. Records both text and
images, opens with `Super+V`, keeps the last 20 entries. Works on
**Cinnamon/X11** and on **GNOME/Wayland** (Ubuntu 24.04).

---

## 0. The fast path: `install.sh`

If you just want it working, the script does everything in this document
automatically — it detects X11 vs Wayland, installs the right helpers, sets up
`/dev/uinput` access for Wayland auto-paste, symlinks the binary, binds `Super+V`
(GNOME **or** Cinnamon), writes autostart, and starts the daemon:

```bash
cd cliphist-rs
./install.sh
```

It is idempotent — safe to re-run. On Wayland it adds you to the `input` group,
so **log out and back in once** afterwards for auto-paste to work. For the
day-to-day rebuild loop after editing code, use `./update.sh` (see section 10).

The rest of this document is the manual, step-by-step path.

---

## 1. Install the build tools

```bash
sudo apt update
sudo apt install cargo build-essential pkg-config libgtk-3-dev
# then the runtime helpers for YOUR session:
#   X11 (Cinnamon):   sudo apt install xdotool xclip
#   Wayland (GNOME):  sudo apt install ydotool wl-clipboard
```

Not sure which session you are on? `echo $XDG_SESSION_TYPE` prints `x11` or
`wayland`.

| Package | Why |
|---|---|
| `cargo` | compiles Rust (pulls in `rustc`; Mint's 1.75 is new enough) |
| `build-essential` | the `cc` linker and a C compiler for the bundled SQLite |
| `pkg-config`, `libgtk-3-dev` | GTK3 headers for the panel |
| `xdotool` (X11) | presses `Ctrl+V` after you pick, and remembers the focused window |
| `ydotool` (Wayland) | presses `Ctrl+V` via `/dev/uinput`, since X11 injection can't reach Wayland apps |

Check with `cargo --version`; it should print `cargo 1.75.0` or newer.

`xclip` (X11) / `wl-copy` (Wayland) are **optional** — only a fallback for when
the daemon is not running.

Recommended fonts for the macOS look (see section 7):

```bash
sudo apt install fonts-inter fonts-jetbrains-mono
```

---

## 2. Build

```bash
tar xzf cliphist-rs.tar.gz
cd cliphist-rs
cargo build --release
```

The first build takes **2-4 minutes** (SQLite, all of gtk-rs, then LTO). When it
finishes you should see:

```
    Finished release [optimized] target(s) in 2m21s
```

The binary lands in `target/release/cliphist`, about 3.8 MB.

> Do not run `cargo update`. `Cargo.lock` pins versions that work with Mint's
> rustc 1.75; newer releases of `indexmap` / `toml_edit` / `serde` require
> `edition2024`, which cargo 1.75 cannot parse.

For a faster test build: `cargo build --release --config 'profile.release.lto=false'`.

---

## 3. Install the binary

Symlink it instead of copying — then every future `cargo build --release`
publishes itself and you never re-copy:

```bash
mkdir -p ~/.local/bin
ln -sfn "$PWD/target/release/cliphist" ~/.local/bin/cliphist
```

(Prefer a real copy? `install -Dm755 target/release/cliphist ~/.local/bin/cliphist`
— but then you must re-copy after every rebuild.)

Verify `~/.local/bin` is on your `PATH`:

```bash
cliphist --help
```

If that reports `command not found`, add it to `~/.zshrc` (or `~/.bashrc` if you
use bash) and open a new terminal:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
```

---

## 4. Set up autostart and the hotkey

```bash
cliphist install
```

This does two things:

- writes `~/.config/autostart/cliphist-lite.desktop` so the daemon starts at login
- binds `Super+V` through `gsettings org.cinnamon.desktop.keybindings`, picking
  the first free `customN` slot so your existing shortcuts are left alone

For a different key: `cliphist install --hotkey '<Super>c'`.

You can review it under **Settings → Keyboard → Shortcuts → Custom Shortcuts**,
where it appears as "Clipboard History".

> **On GNOME (Ubuntu), the hotkey step above does nothing** — it writes a
> *Cinnamon* schema. Either run `./install.sh` (it binds Super+V through the GNOME
> schema), or add it by hand: **Settings → Keyboard → View and Customize
> Shortcuts → Custom Shortcuts → +**, name `cliphist`, command
> `~/.local/bin/cliphist pick --paste`, shortcut `Super+V`.

---

## 4b. Wayland only: enable auto-paste (`ydotool`)

On Wayland, X11 key injection can't reach Wayland apps, so paste goes through
`ydotool`, which writes to `/dev/uinput` at the kernel level. That device is
root-owned, so grant your user access once:

```bash
sudo usermod -aG input "$USER"
echo 'KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"' \
  | sudo tee /etc/udev/rules.d/80-uinput.rules
sudo udevadm control --reload-rules && sudo udevadm trigger
```

**Log out and back in once** so the new `input` group applies. Then run the
`ydotoold` background daemon (the client talks to it over a socket):

```bash
systemctl --user enable --now ydotoold 2>/dev/null || (nohup ydotoold >/dev/null 2>&1 &)
```

Test it: focus a text field and run `ydotool type "ok"` — if `ok` appears, paste
will work. If it errors about permissions, the re-login hasn't happened yet.

On X11 you can skip this whole section — `xdotool` needs no special setup.

---

## 5. Start the daemon

Autostart only takes effect at the next login, so start it by hand this once:

```bash
nohup cliphist daemon >~/.local/share/cliphist-lite/daemon.log 2>&1 &
```

Then check:

```bash
cliphist doctor
```

You want to see a value for `DISPLAY`, a path for your paste helper (`xdotool` on
X11, `ydotool` on Wayland), `daemon = running` and `X11 clipboard = OK`. On
Wayland `DISPLAY` is still set because the daemon uses XWayland — that is
expected and correct.

Copy a few pieces of text and an image, then confirm they were recorded:

```bash
cliphist list
```

Once entries show up, press **Super+V** — the panel opens next to the pointer.

---

## 6. Using the panel

| Key | Action |
|---|---|
| type anything | filter (several words, all must match) |
| `↑` `↓` | move |
| `PgUp` `PgDn` | jump five entries |
| `Enter`, or click a row | copy and auto-paste |
| `Alt+1` … `Alt+9` | pick the Nth entry directly |
| `Ctrl+P`, or the round button on the right | pin / unpin |
| `Delete` | remove the selected entry |
| `Esc`, a click outside, or `Super+V` again | close |

Pinned entries always sort to the top and are **never dropped when the history
fills up**.

---

## 7. Configuration

```bash
cliphist set max-entries 20    # entries to keep (default 20; pinned don't count)
cliphist set blur-close off    # clicking outside does NOT close the panel
cliphist set blur-close on     # back to closing
cliphist set theme light       # default, modelled on the macOS Clipboard panel
cliphist set theme dark        # dark background, #0a84ff accent
cliphist set opacity 0.88      # panel background opacity, 0.30 to 1.0
```

All of these take effect the next time the panel opens; no daemon restart is
needed. Lowering `max-entries` prunes older rows immediately.

Settings live in `~/.config/cliphist-lite/settings.conf`.

### Appearance and fonts

The panel follows the macOS Clipboard panel: translucent background with a 16px
corner radius, an inline search field as the header, and flat rows separated by
hairlines. Each row shows 2.5 lines of text by default — the half-visible last
line signals that the entry continues. Image entries show a small inline
preview. The selected row gets a soft blue tint.

Row height and panel width are the `PREVIEW_LINES` and `W` constants near the
top of `src/ui.rs` (2.5 lines, 470px).

macOS system fonts (SF Pro, SF Mono) are Apple proprietary and licensed only for
Apple platforms, so they are **not available on Linux and cannot be bundled**.
The font stack targets **Inter** (the closest free match, used first) and
**JetBrains Mono**, falling back to SF Pro / SF Mono for anyone who installed
those themselves. Install the free fonts for the intended look:

```bash
sudo apt install fonts-inter fonts-jetbrains-mono
```

To use different fonts, edit `FONT_UI` and `FONT_MONO` at the top of
`src/ui.rs`; colours live in `CSS_LIGHT` / `CSS_DARK` just below.

Rounded corners and opacity need a compositor. Cinnamon runs one by default; if
yours is disabled the panel falls back to square, opaque corners.

### Keeping sensitive content out

`~/.config/cliphist-lite/ignore.txt` holds one regex per line; matching clipboard
text is never recorded. The file is hot-reloaded. A template is included, so you
only need to uncomment:

```
^ghp_[A-Za-z0-9]{36}$
^sk-[A-Za-z0-9]{20,}$
-----BEGIN [A-Z ]*PRIVATE KEY-----
```

Rust's `regex` crate does **not** support lookahead, lookbehind or
backreferences. Patterns such as `(?=...)` are skipped with a warning in the log.

### Pausing

```bash
cliphist pause     # before screen sharing
cliphist resume
```

---

## 8. Other commands

```bash
cliphist list -n 20      # print the history
cliphist get 12          # print entry 12 (images: print the file path)
cliphist copy 12         # put entry 12 on the clipboard
cliphist wipe            # clear everything except pinned entries
cliphist wipe --all      # clear everything
cliphist pick --stay     # open the panel; clicking outside keeps it open
cliphist doctor          # check the environment
cliphist uninstall       # remove autostart
```

---

## 9. Coming from the Python version

The database is compatible and migrates itself, so your history carries over.
Just stop the old one:

```bash
pkill -f 'cliphist.py daemon'
rm -f ~/.config/autostart/cliphist-lite.desktop
cliphist install
nohup cliphist daemon >~/.local/share/cliphist-lite/daemon.log 2>&1 &
```

An old hotkey pointing at the Python script gets overwritten by
`cliphist install`, which recognises any slot whose command contains `cliphist`.

Note the Python build kept 500 text entries and 60 images; this one keeps 20.
The first copy after switching trims down to 20, so pin (`Ctrl+P`) anything you
want to keep first.

---

## 10. Updating later

If you installed with the scripts (or symlinked the binary in section 3), one
command rebuilds and restarts:

```bash
./update.sh            # build --release, then restart the daemon
./update.sh --debug    # much faster unoptimized build while iterating
```

`update.sh` relies on `~/.local/bin/cliphist` being a **symlink** to
`target/release/cliphist`, so rebuilding publishes itself — no copy needed. The
hotkey points at that same path, so it keeps working.

Doing it by hand (symlink already in place):

```bash
cd cliphist-rs && cargo build --release
pkill -f 'cliphist daemon'
nohup cliphist daemon >~/.local/share/cliphist-lite/daemon.log 2>&1 &
```

If you copied the binary instead of symlinking, add
`install -Dm755 target/release/cliphist ~/.local/bin/cliphist` after the build.
No need to re-run `cliphist install` unless the binary path changed.

---

## 11. Troubleshooting

| Symptom | Fix |
|---|---|
| `The system library 'gtk+-3.0' was not found` | install `libgtk-3-dev pkg-config` |
| `linker 'cc' not found` | install `build-essential` |
| `feature 'edition2024' is required` | `Cargo.lock` is missing or was updated — re-extract the tarball |
| `command not found: cargo` | `sudo apt install cargo` |
| Super+V does nothing | run `cliphist doctor`; check Settings → Keyboard → Shortcuts; try `cliphist pick` by hand |
| Panel opens but the history is empty | the daemon is not running: `cliphist doctor`, then read `~/.local/share/cliphist-lite/daemon.log` |
| Picking an entry does not paste (X11) | `xdotool` missing, or the target app does not take `Ctrl+V` (terminals usually want `Ctrl+Shift+V`) |
| Picking an entry does not paste (Wayland) | install `ydotool`; make sure `ydotoold` is running and you re-logged in after joining the `input` group (section 4b); test with `ydotool type ok` |
| Selecting an entry opens **Remote Desktop** settings | you are on Wayland with an old binary that used `xdotool` — rebuild (`./update.sh`); the Wayland build uses `ydotool` and no longer misfires |
| Panel doesn't close when clicking outside (Wayland) | old binary — rebuild; the Wayland build closes on focus-out instead of the (non-working) seat grab |
| Panel appears centred, not near the cursor (Wayland) | expected — GTK can't set a toplevel's absolute position on Wayland |
| Panel closes at the wrong moment | `CLIPHIST_DEBUG=1 cliphist pick` prints click coordinates; or `cliphist set blur-close off` |
| `daemon is already running (pid N)` | it is; `kill N` first if you want to restart it |

Daemon log: `~/.local/share/cliphist-lite/daemon.log` when started with the `nohup` line above.

---

## 12. Where the data lives

```
~/.local/share/cliphist-lite/history.db     SQLite, chmod 600
~/.local/share/cliphist-lite/images/        images, named by sha256, chmod 600
~/.config/cliphist-lite/settings.conf       settings
~/.config/cliphist-lite/ignore.txt          exclusion regexes
```

`history.db` is **plaintext**. It is chmod 600 inside `$HOME`, but any process
running as your user can read it — treat it as a secrets file. To remove
everything:

```bash
cliphist uninstall
pkill -f 'cliphist daemon'
rm -rf ~/.local/share/cliphist-lite ~/.config/cliphist-lite ~/.local/bin/cliphist
```

The hotkey has to be deleted by hand in Settings → Keyboard → Shortcuts.
