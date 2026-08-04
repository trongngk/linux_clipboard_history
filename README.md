# cliphist — clipboard history for Linux (X11 & Wayland)

> Step-by-step setup: [INSTALL.md](INSTALL.md) · Quick guide: [QUICKSTART.md](QUICKSTART.md)

A Win+V style clipboard history. Records text and images, opens a panel with
`Super+V`, keeps the last 20 entries. Works on **Cinnamon/X11** and on
**GNOME/Wayland** (Ubuntu 24.04): the daemon owns the X selection and Mutter
bridges it to Wayland apps through XWayland.

## Quick install

The scripts detect X11 vs Wayland and install the right helpers, symlink the
binary, bind `Super+V`, and start the daemon:

```bash
./install.sh        # one-time full setup (may ask for sudo)
./update.sh         # rebuild + restart daemon after editing code (add --debug for a fast build)
```

The rest of this file is the manual path and the design notes.

## Build

```bash
sudo apt install build-essential pkg-config libgtk-3-dev
# X11 (Cinnamon):   sudo apt install xdotool xclip
# Wayland (GNOME):  sudo apt install ydotool wl-clipboard
cargo build --release
```

The binary lands in `target/release/cliphist` (~3.8 MB). Mint's own toolchain
(`sudo apt install cargo`, rustc 1.75) is new enough — the versions in
`Cargo.lock` are pinned for it, so avoid `cargo update` unless you have a newer
rustup toolchain.

## Install (manual)

```bash
ln -sfn "$PWD/target/release/cliphist" ~/.local/bin/cliphist   # symlink: rebuilds need no re-copy
cliphist install                 # autostart + Super+V hotkey (Cinnamon gsettings)
nohup cliphist daemon >~/.local/share/cliphist-lite/daemon.log 2>&1 &
cliphist doctor                  # check the environment
```

Different key: `cliphist install --hotkey '<Super>c'`.

`cliphist install` binds the hotkey through **Cinnamon** gsettings only. On
**GNOME** either run `./install.sh` (it binds Super+V via the GNOME schema) or
add it by hand in **Settings → Keyboard → Custom Shortcuts** with command
`~/.local/bin/cliphist pick --paste`.

### Auto-paste helpers

Picking an entry sets the clipboard and then synthesizes `Ctrl+V`:

| Session | Tool used | Notes |
|---|---|---|
| X11 | `xdotool` | also re-focuses the window that was active before the panel |
| Wayland (GNOME) | `ydotool` | injects via `/dev/uinput`; needs `ydotoold` running and `input`-group access to `/dev/uinput` |
| Wayland (wlroots: sway, Hyprland) | `wtype` | virtual-keyboard protocol; **not** supported by GNOME |

`install.sh` sets up the `input` group + a udev rule for `/dev/uinput` and starts
`ydotoold`. After the first run, **log out and back in once** so the group takes
effect. `xclip` (X11) / `wl-copy` (Wayland) are only used as a fallback when the
daemon is not running.

## Commands

| Command | Purpose |
|---|---|
| `cliphist daemon [--interval MS]` | background: record history and own the X selection (default 400ms) |
| `cliphist pick [--paste] [--stay]` | open the panel; `--paste` auto-pastes; `--stay` ignores clicks outside |
| `cliphist list [-n N]` | print the history |
| `cliphist get <id>` | print an entry (images: the file path) |
| `cliphist copy <id>` | put an entry on the clipboard |
| `cliphist wipe [--all]` | clear history; `--all` includes pinned entries |
| `cliphist pause` / `resume` | stop / resume recording |
| `cliphist set blur-close on\|off` | whether clicking outside closes the panel |
| `cliphist set max-entries N` | entries to keep (default 20); applied immediately |
| `cliphist set theme light\|dark` | panel appearance (default light) |
| `cliphist set opacity 0.3-1.0` | panel background opacity (default 0.88) |
| `cliphist doctor` | check DISPLAY, helper binaries, daemon, database |
| `cliphist uninstall` | remove autostart |

## Panel keys

Type to filter, `↑↓` to move, `PgUp/PgDn` to jump five, `Enter` or a click to
copy, `Alt+1..9` to pick directly, `Ctrl+P` to pin, `Delete` to remove, `Esc` to
close. Pressing `Super+V` again closes it too, and so does a click outside.

Pinned entries sort to the top and are never trimmed.

## Appearance

Modelled on the macOS Clipboard panel: translucent background with a 16px corner
radius, an inline search field as the header with a `•••` button, and flat rows
separated by hairlines. Each row shows **2.5 lines** of content by default, so a
half-visible last line hints that there is more, with a small pin button on the
right — clicking the row already copies, so the button is used for pinning
instead. Image entries show an inline preview (up to 240×104). The selected row
gets a soft `rgba(0,122,255,0.14)` tint rather than a solid fill.

A GtkLabel only renders whole lines and `set_size_request` is a minimum rather
than a maximum, so the label sits in a scrolled window with scrolling switched
off, which clips it to an exact pixel height — that is what makes half a line
possible. Line height comes from the widget's own font metrics, so it follows
whichever font ends up being used.

Row height and panel width are `PREVIEW_LINES` and `W` near the top of
`src/ui.rs` (2.5 lines, 470px). Note that a non-resizable GTK window ignores
`set_default_size` and takes the natural size of its content, so the width is
forced with `set_size_request`.

```bash
cliphist set theme dark
cliphist set opacity 0.75
```

Rounded corners and opacity need a compositor; without one the panel falls back
to square and opaque.

The font stack targets Inter and JetBrains Mono (free, install them below),
falling back to SF Pro / SF Mono for anyone who has them — SF is Apple
proprietary and cannot be redistributed on Linux:

```bash
sudo apt install fonts-inter fonts-jetbrains-mono
```

Fonts: `FONT_UI` / `FONT_MONO` at the top of `src/ui.rs`. Colours: `CSS_LIGHT` /
`CSS_DARK` just below.

## Architecture

One important difference from the earlier Python build: **X11 requires the
process owning a selection to stay alive**. The Python version delegated that to
`xclip`, which forks into the background. Here the daemon owns the selection
itself:

```
picker (lives ~2s)  ──unix socket──>  daemon (long-lived)  ──> X selection owner
   $XDG_RUNTIME_DIR/cliphist-lite.sock      "SET <id>"
```

If the daemon is not running the picker falls back to `xclip` (X11) or `wl-copy`
(Wayland), which are therefore optional rather than required. On Wayland the
daemon's X selection is mirrored to Wayland clients by Mutter's XWayland bridge,
so history capture and paste both keep working without any Wayland-native
clipboard code.

Clipboard changes are detected with two cheap signals instead of re-reading the
content:

1. the selection's `TIMESTAMP` target
2. the window owning the clipboard (`GetSelectionOwner`)

Both are needed because not every application answers `TIMESTAMP` — `xclip` is
one that does not. This keeps multi-megabyte images from being re-read on every
poll.

Reading uses `TARGETS` to decide what to fetch rather than probing blindly, with
generous timeouts (4s text, 6s images): Java applications such as Burp Suite are
slow to answer selection requests, and large payloads go through the multi-round
INCR protocol.

On **X11** the panel grabs pointer and keyboard the way GTK does for menus and
popovers, so a click outside is delivered to the panel with root coordinates
outside its frame — that is what closes it. On **Wayland** a GDK seat grab on a
normal toplevel does not deliver outside clicks (only `xdg_popup` surfaces may
grab) yet can still report success, which would suppress the fallback and leave
the panel stuck open; so the grab is skipped there and `focus-out-event` handles
closing instead. The grab, when taken, is released and the window destroyed
before `Ctrl+V` is sent, and the panel closes itself after 120 seconds as a
safety net.

Session detection is `util::is_wayland()` (`WAYLAND_DISPLAY` /
`XDG_SESSION_TYPE`); the paste path lives in `clip::auto_paste`. Note GTK cannot
place a toplevel at absolute coordinates on Wayland, so the panel appears where
the compositor puts it (usually centred) rather than next to the pointer.

## Data

```
~/.local/share/cliphist-lite/history.db     SQLite (chmod 600)
~/.local/share/cliphist-lite/images/        images, named by sha256 (chmod 600)
~/.config/cliphist-lite/settings.conf       settings
~/.config/cliphist-lite/ignore.txt          exclusion regexes, hot-reloaded
```

The schema is **compatible with the Python version** and migrates itself.

Limits: **20 entries** (text and images share the budget), images up to 20 MB.
Pinned entries are exempt and never trimmed; deleting an entry also cleans up
orphaned image files.

```bash
cliphist set max-entries 50
```

The default lives in `DEFAULT_MAX_ENTRIES` in `src/db.rs`.

## ignore.txt

One regex per line; matching content is not recorded. Useful when copying
tokens, cookies or private keys during testing. Note that Rust's `regex` crate
has **no lookahead, lookbehind or backreferences** — patterns like `(?=...)` are
skipped with a warning.

Before screen sharing: `cliphist pause`.

## Security note

`history.db` is **plaintext**. It sits in `$HOME` with chmod 600, but any
process running as your user can read it — treat it as a secrets file.

## Debugging

`CLIPHIST_DEBUG=1 cliphist pick` prints the coordinates of every click along
with the panel frame and whether it counted as outside.
