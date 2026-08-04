use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const APP: &str = "cliphist-lite";

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".local/share"));
    base.join(APP)
}

pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"));
    base.join(APP)
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/root"))
}

pub fn db_path() -> PathBuf {
    data_dir().join("history.db")
}
pub fn image_dir() -> PathBuf {
    data_dir().join("images")
}
pub fn pause_flag() -> PathBuf {
    data_dir().join("paused")
}
pub fn ignore_file() -> PathBuf {
    config_dir().join("ignore.txt")
}
pub fn settings_file() -> PathBuf {
    config_dir().join("settings.conf")
}

/// Read one key from settings.conf (key=value format, # starts a comment).
pub fn setting(key: &str) -> Option<String> {
    let text = std::fs::read_to_string(settings_file()).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

pub fn setting_bool(key: &str, default: bool) -> bool {
    match setting(key).as_deref() {
        Some("1") | Some("true") | Some("on") | Some("yes") => true,
        Some("0") | Some("false") | Some("off") | Some("no") => false,
        _ => default,
    }
}

pub fn setting_usize(key: &str, default: usize) -> usize {
    setting(key)
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(default)
}

pub fn set_setting(key: &str, value: &str) -> std::io::Result<()> {
    ensure_dirs()?;
    let path = settings_file();
    let mut lines: Vec<String> = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .map(|l| l.to_string())
        .collect();
    let mut found = false;
    for line in lines.iter_mut() {
        let t = line.trim();
        if t.starts_with('#') || t.is_empty() {
            continue;
        }
        if let Some((k, _)) = t.split_once('=') {
            if k.trim() == key {
                *line = format!("{}={}", key, value);
                found = true;
            }
        }
    }
    if !found {
        lines.push(format!("{}={}", key, value));
    }
    fs::write(&path, lines.join("\n") + "\n")
}

pub fn pick_lock() -> PathBuf {
    data_dir().join("picker.lock")
}
pub fn daemon_lock() -> PathBuf {
    data_dir().join("daemon.lock")
}

pub fn socket_path() -> PathBuf {
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(d) => PathBuf::from(d).join("cliphist-lite.sock"),
        None => PathBuf::from(format!("/tmp/cliphist-lite-{}.sock", unsafe { libc::getuid() })),
    }
}

const IGNORE_TEMPLATE: &str = "\
# One regex per line. Clipboard text matching any of them is NOT recorded.
# This file is hot-reloaded; no need to restart the daemon.
# Uncomment the lines below if you want (keeps secrets out of the history):
# ^ghp_[A-Za-z0-9]{36}$
# ^sk-[A-Za-z0-9]{20,}$
# -----BEGIN [A-Z ]*PRIVATE KEY-----
# ^eyJ[A-Za-z0-9_-]{10,}\\.[A-Za-z0-9_-]{10,}\\.
";

pub fn ensure_dirs() -> std::io::Result<()> {
    fs::create_dir_all(image_dir())?;
    fs::create_dir_all(config_dir())?;
    chmod(&data_dir(), 0o700);
    chmod(&image_dir(), 0o700);
    if !ignore_file().exists() {
        fs::write(ignore_file(), IGNORE_TEMPLATE)?;
    }
    Ok(())
}

pub fn chmod(path: &std::path::Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
}

pub fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn human_age(ts: f64) -> String {
    let d = (now() - ts).max(0.0) as u64;
    match d {
        0..=59 => format!("{}s", d),
        60..=3599 => format!("{}m", d / 60),
        3600..=86399 => format!("{}h", d / 3600),
        _ => format!("{}d", d / 86400),
    }
}

pub fn human_bytes(n: usize) -> String {
    if n < 1024 {
        format!("{} B", n)
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    }
}

/// Collapse text into a single line for display.
pub fn preview(text: &str, width: usize) -> String {
    let joined: String = text
        .trim()
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ⏎ ");
    let flat = joined.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate(&flat, width)
}

pub fn truncate(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(width.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Read image dimensions straight from the header; no full decode needed.
pub fn image_size(data: &[u8]) -> Option<(u32, u32)> {
    let be32 = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
    let le16 = |b: &[u8]| u16::from_le_bytes([b[0], b[1]]) as u32;
    let le32 = |b: &[u8]| u32::from_le_bytes([b[0], b[1], b[2], b[3]]);

    if data.len() > 24 && data.starts_with(b"\x89PNG\r\n\x1a\n") && &data[12..16] == b"IHDR" {
        return Some((be32(&data[16..20]), be32(&data[20..24])));
    }
    if data.len() > 10 && (data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a")) {
        return Some((le16(&data[6..8]), le16(&data[8..10])));
    }
    if data.len() > 26 && data.starts_with(b"BM") {
        return Some((le32(&data[18..22]), le32(&data[22..26])));
    }
    if data.len() > 4 && data.starts_with(b"\xff\xd8") {
        let mut i = 2usize;
        while i + 9 < data.len() {
            if data[i] != 0xFF {
                i += 1;
                continue;
            }
            let m = data[i + 1];
            if (0xC0..=0xCF).contains(&m) && m != 0xC4 && m != 0xC8 && m != 0xCC {
                let h = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
                let w = u16::from_be_bytes([data[i + 7], data[i + 8]]) as u32;
                return Some((w, h));
            }
            let seg = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
            i += 2 + seg;
        }
    }
    None
}

pub fn ext_for(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/bmp" => "bmp",
        "image/gif" => "gif",
        _ => "bin",
    }
}

/// Exclusive non-blocking flock. The lock holds as long as `File` is alive.
pub struct Lock {
    _file: File,
}

pub enum LockResult {
    Acquired(Lock),
    Held(Option<i32>),
}

pub fn acquire_lock(path: &std::path::Path) -> std::io::Result<LockResult> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)?;
    let rc = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        let mut s = String::new();
        f.seek(SeekFrom::Start(0))?;
        f.read_to_string(&mut s)?;
        return Ok(LockResult::Held(s.trim().parse::<i32>().ok()));
    }
    f.seek(SeekFrom::Start(0))?;
    f.set_len(0)?;
    write!(f, "{}", std::process::id())?;
    f.flush()?;
    Ok(LockResult::Acquired(Lock { _file: f }))
}

pub fn kill(pid: i32) {
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
}

/// True when running under a Wayland session. X11 input tools (xdotool) cannot
/// see the focused window or inject keystrokes into Wayland-native apps there,
/// and GDK seat grabs on a normal toplevel do not deliver outside clicks, so
/// both auto-paste and click-outside-to-close need a different path.
pub fn is_wayland() -> bool {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return true;
    }
    matches!(std::env::var("XDG_SESSION_TYPE").as_deref(), Ok("wayland"))
}

pub fn which(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(bin))
        .find(|p| p.is_file())
}
