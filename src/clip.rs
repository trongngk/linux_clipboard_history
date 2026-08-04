use crate::db::Entry;
use crate::util::*;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;
use x11_clipboard::Clipboard as X11Clipboard;
use x11rb::protocol::xproto::ConnectionExt as _;

pub const IMAGE_MIMES: [&str; 4] = ["image/png", "image/jpeg", "image/bmp", "image/gif"];

/// Text targets in priority order. Not every application offers UTF8_STRING;
/// Java apps such as Burp Suite often provide text/plain;charset=utf-8 too,
/// sometimes only that.
pub const TEXT_MIMES: [&str; 3] = ["text/plain;charset=utf-8", "text/plain", "STRING"];

/// Reading content needs a far more generous budget than change detection:
/// Java applications answer selection requests slowly, and large payloads (an
/// HTTP request with a body) force X11 through the multi-round INCR protocol.
const READ_TEXT_MS: u64 = 4000;
const READ_IMAGE_MS: u64 = 6000;

/// What was read off the clipboard.
pub enum Grab {
    Text(String),
    Image(Vec<u8>, &'static str),
    Empty,
}

pub struct Clip {
    c: X11Clipboard,
    ts_atom: u32,
    targets_atom: u32,
    text_atoms: Vec<u32>,
    image_atoms: Vec<(&'static str, u32)>,
}

impl Clip {
    pub fn new() -> Result<Self, String> {
        let c = X11Clipboard::new().map_err(|e| format!("could not open the X11 clipboard: {e}"))?;
        let ts_atom = c
            .getter
            .get_atom("TIMESTAMP")
            .map_err(|e| format!("could not intern the TIMESTAMP atom: {e}"))?;
        let mut image_atoms = Vec::new();
        for m in IMAGE_MIMES {
            if let Ok(a) = c.getter.get_atom(m) {
                image_atoms.push((m, a));
            }
        }
        let mut text_atoms = vec![c.getter.atoms.utf8_string];
        for m in TEXT_MIMES {
            if let Ok(a) = c.getter.get_atom(m) {
                text_atoms.push(a);
            }
        }
        let targets_atom = c.getter.atoms.targets;
        Ok(Self {
            c,
            ts_atom,
            targets_atom,
            text_atoms,
            image_atoms,
        })
    }

    /// Targets currently offered by the selection owner, as atom ids.
    /// Comparing ids avoids having to translate atoms back into names.
    pub fn targets(&self) -> Vec<u32> {
        match self.load(self.targets_atom, 1500) {
            Some(raw) => raw
                .chunks_exact(4)
                .map(|b| u32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
                .collect(),
            None => Vec::new(),
        }
    }

    /// Translate atom ids into names; only used by `doctor`.
    pub fn target_names(&self) -> Vec<String> {
        self.targets()
            .into_iter()
            .filter_map(|a| {
                let r = x11rb::protocol::xproto::get_atom_name(&self.c.getter.connection, a)
                    .ok()?
                    .reply()
                    .ok()?;
                Some(String::from_utf8_lossy(&r.name).to_string())
            })
            .collect()
    }

    fn load(&self, target: u32, ms: u64) -> Option<Vec<u8>> {
        self.c
            .load(
                self.c.getter.atoms.clipboard,
                target,
                self.c.getter.atoms.property,
                Duration::from_millis(ms),
            )
            .ok()
            .filter(|v| !v.is_empty())
    }

    /// The selection TIMESTAMP: a change means the clipboard changed. Much
    /// cheaper than reading the content itself.
    pub fn signature(&self) -> Option<Vec<u8>> {
        self.load(self.ts_atom, 300)
    }

    /// The window currently owning the clipboard. One cheap X round-trip; a
    /// new owner means another application just copied. Needed because not
    /// every client answers TIMESTAMP - xclip, for one, does not.
    pub fn owner(&self) -> Option<u32> {
        self.c
            .getter
            .connection
            .get_selection_owner(self.c.getter.atoms.clipboard)
            .ok()?
            .reply()
            .ok()
            .map(|r| r.owner)
            .filter(|w| *w != 0)
    }

    /// Combined token used to tell whether the clipboard changed.
    pub fn change_token(&self) -> (Option<Vec<u8>>, Option<u32>) {
        (self.signature(), self.owner())
    }

    /// Prefer text; fall through to images when there is none.
    ///
    /// Ask for TARGETS first instead of probing blindly: every attempt at a
    /// target the owner does not provide costs a full timeout, and those add
    /// up to seconds of dead time. Only when the owner offers no TARGETS at
    /// all (some older clients) does this fall back to probing in turn.
    pub fn grab(&self) -> Grab {
        let targets = self.targets();

        if targets.is_empty() {
            return self.grab_blind();
        }

        for atom in &self.text_atoms {
            if targets.contains(atom) {
                if let Some(raw) = self.load(*atom, READ_TEXT_MS) {
                    if let Some(s) = decode_text(raw) {
                        return Grab::Text(s);
                    }
                }
            }
        }
        for (mime, atom) in &self.image_atoms {
            if targets.contains(atom) {
                if let Some(data) = self.load(*atom, READ_IMAGE_MS) {
                    return Grab::Image(data, mime);
                }
            }
        }
        Grab::Empty
    }

    fn grab_blind(&self) -> Grab {
        for atom in &self.text_atoms {
            if let Some(raw) = self.load(*atom, READ_TEXT_MS) {
                if let Some(s) = decode_text(raw) {
                    return Grab::Text(s);
                }
            }
        }
        for (mime, atom) in &self.image_atoms {
            if let Some(data) = self.load(*atom, READ_IMAGE_MS) {
                return Grab::Image(data, mime);
            }
        }
        Grab::Empty
    }

    /// Put an entry on the clipboard. X11 requires the process owning the
    /// selection to stay alive, so this is only usable from the daemon.
    pub fn put(&self, entry: &Entry) -> Result<(), String> {
        if entry.is_image() {
            let path = entry.path.clone().ok_or("image entry has no path")?;
            let data = std::fs::read(&path).map_err(|e| format!("reading {path}: {e}"))?;
            let mime = entry.mime();
            let atom = self
                .c
                .setter
                .get_atom(&mime)
                .map_err(|e| format!("atom {mime}: {e}"))?;
            self.c
                .store(self.c.setter.atoms.clipboard, atom, data)
                .map_err(|e| format!("storing image: {e}"))
        } else {
            self.c
                .store(
                    self.c.setter.atoms.clipboard,
                    self.c.setter.atoms.utf8_string,
                    entry.content.as_bytes(),
                )
                .map_err(|e| format!("store text: {e}"))
        }
    }
}

/// Try UTF-8 first; the X11 STRING target is latin-1 by specification, so fall
/// back to that rather than losing the content over one odd byte.
fn decode_text(raw: Vec<u8>) -> Option<String> {
    let s = match String::from_utf8(raw) {
        Ok(s) => s,
        Err(e) => e.as_bytes().iter().map(|b| *b as char).collect(),
    };
    if s.trim().is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Fallback when the daemon is not running: let xclip hold the selection, as
/// it forks into the background by itself.
pub fn xclip_put(entry: &Entry) -> Result<(), String> {
    // On Wayland xclip talks only to XWayland; wl-copy owns the real Wayland
    // selection and, like xclip, forks into the background to hold it.
    if is_wayland() && which("wl-copy").is_some() {
        return wl_copy_put(entry);
    }
    if which("xclip").is_none() {
        if is_wayland() {
            return Err(
                "the daemon is not running; install wl-clipboard (wl-copy) for Wayland".into(),
            );
        }
        return Err("the daemon is not running and xclip is not installed".into());
    }
    let (data, mime): (Vec<u8>, String) = if entry.is_image() {
        let path = entry.path.clone().ok_or("image entry has no path")?;
        (
            std::fs::read(&path).map_err(|e| e.to_string())?,
            entry.mime(),
        )
    } else {
        (
            entry.content.as_bytes().to_vec(),
            "UTF8_STRING".to_string(),
        )
    };
    let mut ch = Command::new("xclip")
        .args(["-selection", "clipboard", "-t", &mime, "-i"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    ch.stdin
        .as_mut()
        .ok_or("could not open xclip stdin")?
        .write_all(&data)
        .map_err(|e| e.to_string())?;
    ch.wait().map_err(|e| e.to_string())?;
    Ok(())
}

/// Wayland counterpart of `xclip_put`: let wl-copy hold the selection.
fn wl_copy_put(entry: &Entry) -> Result<(), String> {
    let (data, mime): (Vec<u8>, String) = if entry.is_image() {
        let path = entry.path.clone().ok_or("image entry has no path")?;
        (
            std::fs::read(&path).map_err(|e| e.to_string())?,
            entry.mime(),
        )
    } else {
        (
            entry.content.as_bytes().to_vec(),
            "text/plain;charset=utf-8".to_string(),
        )
    };
    let mut ch = Command::new("wl-copy")
        .args(["--type", &mime])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    ch.stdin
        .as_mut()
        .ok_or("could not open wl-copy stdin")?
        .write_all(&data)
        .map_err(|e| e.to_string())?;
    ch.wait().map_err(|e| e.to_string())?;
    Ok(())
}

/// Remember the focused window before the panel opens so the paste lands in
/// the right place.
pub fn active_window() -> Option<String> {
    // Under Wayland xdotool cannot see the focused Wayland window (it returns a
    // stale XWayland id), and auto-paste there does not use it anyway.
    if is_wayland() {
        return None;
    }
    which("xdotool")?;
    let out = Command::new("xdotool").arg("getactivewindow").output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.chars().all(|c| c.is_ascii_digit()) && !s.is_empty() {
        Some(s)
    } else {
        None
    }
}

pub fn auto_paste(win: Option<String>) {
    if is_wayland() {
        auto_paste_wayland();
    } else {
        auto_paste_x11(win);
    }
}

fn auto_paste_x11(win: Option<String>) {
    if which("xdotool").is_none() {
        eprintln!("[{APP}] xdotool missing, cannot auto-paste: sudo apt install xdotool");
        return;
    }
    if let Some(w) = win {
        let _ = Command::new("xdotool")
            .args(["windowactivate", "--sync", &w])
            .output();
    }
    std::thread::sleep(Duration::from_millis(150));
    let _ = Command::new("xdotool")
        .args(["key", "--clearmodifiers", "ctrl+v"])
        .status();
}

/// Wayland has no equivalent of `xdotool getactivewindow`, and X11 key
/// injection does not reach Wayland-native apps. The compositor returns focus
/// to the previously active window by itself once our panel is destroyed, so
/// here we only have to synthesize Ctrl+V.
///
/// `ydotool` injects through /dev/uinput at the kernel level, so it works on
/// GNOME/Mutter (which is what Ubuntu ships). `wtype` uses the virtual-keyboard
/// Wayland protocol and works on wlroots compositors (sway, Hyprland) but NOT
/// GNOME, so it is only a secondary attempt.
fn auto_paste_wayland() {
    // Let the compositor move focus back to the target window first.
    std::thread::sleep(Duration::from_millis(150));

    if which("ydotool").is_some() {
        // Key codes: leftctrl=29, v=47. Press ctrl, tap v, release ctrl.
        let ok = Command::new("ydotool")
            .args(["key", "29:1", "47:1", "47:0", "29:0"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return;
        }
        eprintln!(
            "[{APP}] ydotool failed - is the ydotoold daemon running and do you \
             have access to /dev/uinput? Check: systemctl status ydotool"
        );
    }

    if which("wtype").is_some() {
        let ok = Command::new("wtype")
            .args(["-M", "ctrl", "-k", "v", "-m", "ctrl"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return;
        }
        eprintln!(
            "[{APP}] wtype failed - GNOME does not implement the virtual-keyboard \
             protocol; install ydotool instead"
        );
    }

    eprintln!(
        "[{APP}] no Wayland auto-paste tool found. Install ydotool:\n  \
         sudo apt install ydotool && sudo systemctl enable --now ydotool"
    );
}
