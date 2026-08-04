mod clip;
mod daemon;
mod db;
mod ipc;
mod ui;
mod util;

use std::process::Command;
use std::rc::Rc;
use util::*;

const HELP: &str = "\
cliphist - clipboard history for Linux (X11 & Wayland), like Win+V

    cliphist install [--hotkey <Super>v]   set up autostart + Cinnamon hotkey
    cliphist daemon [--interval MS]        run in background, record + own clipboard
    cliphist pick [--paste] [--stay]       open the panel (--stay: clicking outside keeps it open)
    cliphist set blur-close on|off         whether clicking outside closes the panel
    cliphist set max-entries N             how many entries to keep (default 20)
    cliphist set theme light|dark          panel appearance (default light)
    cliphist set opacity 0.3-1.0           panel background opacity (default 0.88)
    cliphist list [-n N]                   print the history
    cliphist get <id>                      print an entry (images: print the file path)
    cliphist copy <id>                     put an entry on the clipboard
    cliphist wipe [--all]                  clear history (--all: pinned entries too)
    cliphist pause | resume                stop / resume recording
    cliphist doctor                        check the environment
    cliphist uninstall                     remove autostart

Auto-paste uses xdotool on X11 and ydotool on Wayland (GNOME). The clipboard
fallback (xclip on X11, wl-copy on Wayland) is only needed when the daemon is
not running.
";

fn main() {
    // Rust ignores SIGPIPE by default, so `cliphist list | head -1` panics once
    // head closes the pipe. Restore normal Unix behaviour: die quietly.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("help");
    let rest: &[String] = if args.is_empty() { &[] } else { &args[1..] };

    let code = match cmd {
        "daemon" => cmd_daemon(rest),
        "pick" => cmd_pick(rest),
        "list" => cmd_list(rest),
        "get" => cmd_get(rest),
        "copy" => cmd_copy(rest),
        "wipe" => cmd_wipe(rest),
        "set" => cmd_set(rest),
        "pause" => cmd_pause(),
        "resume" => cmd_resume(),
        "doctor" => cmd_doctor(),
        "install" => cmd_install(rest),
        "uninstall" => cmd_uninstall(),
        "-h" | "--help" | "help" => {
            print!("{HELP}");
            0
        }
        other => {
            eprintln!("unknown command: {other}\n");
            print!("{HELP}");
            2
        }
    };
    std::process::exit(code);
}

fn flag_value(args: &[String], name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    args.get(i + 1).cloned()
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn positional(args: &[String]) -> Option<&String> {
    args.iter().find(|a| !a.starts_with('-'))
}

fn cmd_daemon(args: &[String]) -> i32 {
    let interval = flag_value(args, "--interval")
        .or_else(|| flag_value(args, "-i"))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(400);
    match daemon::run(interval) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("[{APP}] {e}");
            1
        }
    }
}

fn cmd_pick(args: &[String]) -> i32 {
    // A second instance closes the open panel - the Win+V style toggle
    let _lock = match acquire_lock(&pick_lock()) {
        Ok(LockResult::Acquired(l)) => l,
        Ok(LockResult::Held(pid)) => {
            if let Some(p) = pid {
                kill(p);
            }
            return 0;
        }
        Err(e) => {
            eprintln!("[{APP}] lock error: {e}");
            return 1;
        }
    };

    let con = match db::open() {
        Ok(c) => Rc::new(c),
        Err(e) => {
            eprintln!("[{APP}] could not open the database: {e}");
            return 1;
        }
    };
    let entries = db::fetch(&con, 300).unwrap_or_default();
    if entries.is_empty() {
        eprintln!("[{APP}] history is empty - is the daemon running? (cliphist daemon)");
        return 1;
    }

    // --stay overrides the stored setting; default comes from settings.conf
    let blur_close = if has_flag(args, "--stay") || has_flag(args, "--no-blur-close") {
        false
    } else {
        setting_bool("blur_close", true)
    };

    let target = clip::active_window();
    let chosen = match ui::pick(con, entries, blur_close) {
        Some(e) => e,
        None => return 0,
    };

    if let Err(e) = put(&chosen) {
        eprintln!("[{APP}] {e}");
        return 1;
    }
    if has_flag(args, "--paste") {
        clip::auto_paste(target);
    }
    0
}

/// The daemon owns the X selection, so ask it to set the clipboard; if it is
/// not running, fall back to xclip.
fn put(entry: &db::Entry) -> Result<(), String> {
    match ipc::client_set(entry.id) {
        Ok(()) => Ok(()),
        Err(_) => clip::xclip_put(entry),
    }
}

fn cmd_list(args: &[String]) -> i32 {
    let n = flag_value(args, "-n")
        .or_else(|| flag_value(args, "--number"))
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(25);
    let con = match db::open() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    for e in db::fetch(&con, n).unwrap_or_default() {
        println!(
            "{:>5} {} {:>4}  {}",
            e.id,
            if e.pinned { "★" } else { " " },
            human_age(e.ts),
            e.label(100)
        );
    }
    0
}

fn cmd_get(args: &[String]) -> i32 {
    let id = match positional(args).and_then(|s| s.parse::<i64>().ok()) {
        Some(i) => i,
        None => {
            eprintln!("an id is required, e.g. cliphist get 12");
            return 2;
        }
    };
    let con = match db::open() {
        Ok(c) => c,
        Err(_) => return 1,
    };
    match db::get(&con, id) {
        Ok(Some(e)) => {
            if e.is_image() {
                println!("{}", e.path.unwrap_or_default());
            } else {
                print!("{}", e.content);
            }
            0
        }
        _ => {
            eprintln!("no entry with id={id}");
            1
        }
    }
}

fn cmd_copy(args: &[String]) -> i32 {
    let id = match positional(args).and_then(|s| s.parse::<i64>().ok()) {
        Some(i) => i,
        None => {
            eprintln!("an id is required, e.g. cliphist copy 12");
            return 2;
        }
    };
    let con = match db::open() {
        Ok(c) => c,
        Err(_) => return 1,
    };
    match db::get(&con, id) {
        Ok(Some(e)) => match put(&e) {
            Ok(()) => 0,
            Err(err) => {
                eprintln!("[{APP}] {err}");
                1
            }
        },
        _ => {
            eprintln!("no entry with id={id}");
            1
        }
    }
}

fn cmd_wipe(args: &[String]) -> i32 {
    let con = match db::open() {
        Ok(c) => c,
        Err(_) => return 1,
    };
    let all = has_flag(args, "--all");
    match db::wipe(&con, all) {
        Ok(()) => {
            println!(
                "History cleared.{}",
                if all { "" } else { " (pinned entries kept)" }
            );
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

fn cmd_set(args: &[String]) -> i32 {
    let key = args.first().map(|s| s.as_str()).unwrap_or("");
    let val = args.get(1).map(|s| s.as_str()).unwrap_or("");
    match (key, val) {
        ("blur-close", v @ ("on" | "off")) => {
            let on = v == "on";
            if let Err(e) = set_setting("blur_close", if on { "true" } else { "false" }) {
                eprintln!("could not write settings: {}", e);
                return 1;
            }
            println!(
                "blur-close = {} - clicking outside {} the panel.",
                v,
                if on { "closes" } else { "does NOT close" }
            );
            println!("File: {}", settings_file().display());
            0
        }
        ("theme", v @ ("light" | "dark")) => {
            if let Err(e) = set_setting("theme", v) {
                eprintln!("could not write settings: {}", e);
                return 1;
            }
            println!("theme = {}", v);
            0
        }
        ("theme", other) => {
            eprintln!("value must be light or dark (got {:?})", other);
            2
        }
        ("opacity", v) => match v.parse::<f64>() {
            Ok(n) if (0.30..=1.0).contains(&n) => {
                if let Err(e) = set_setting("opacity", &format!("{:.2}", n)) {
                    eprintln!("could not write settings: {}", e);
                    return 1;
                }
                println!("opacity = {:.2}", n);
                if n < 1.0 {
                    println!("(only visible while a compositor runs - Cinnamon has one by default)");
                }
                0
            }
            _ => {
                eprintln!("value must be between 0.30 and 1.0, e.g. cliphist set opacity 0.85");
                2
            }
        },
        ("max-entries", v) => match v.parse::<usize>() {
            Ok(n) if n > 0 => {
                if let Err(e) = set_setting("max_entries", &n.to_string()) {
                    eprintln!("could not write settings: {}", e);
                    return 1;
                }
                // apply immediately to the existing rows
                match db::open().and_then(|c| db::trim(&c).map(|_| c)) {
                    Ok(con) => {
                        let left: i64 = con
                            .query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))
                            .unwrap_or(0);
                        println!("max-entries = {} ({} entries remaining)", n, left);
                    }
                    Err(e) => eprintln!("trim failed: {}", e),
                }
                println!("File: {}", settings_file().display());
                0
            }
            _ => {
                eprintln!("a positive integer is required, e.g. cliphist set max-entries 20");
                2
            }
        },
        ("blur-close", other) => {
            eprintln!("value must be on or off (got {:?})", other);
            2
        }
        _ => {
            eprintln!("usage:");
            eprintln!("  cliphist set blur-close on|off");
            eprintln!("  cliphist set max-entries N");
            eprintln!("  cliphist set theme light|dark");
            eprintln!("  cliphist set opacity 0.3-1.0");
            2
        }
    }
}

fn cmd_pause() -> i32 {
    let _ = ensure_dirs();
    let _ = std::fs::write(pause_flag(), b"");
    println!("Clipboard recording paused. Resume with: cliphist resume");
    0
}

fn cmd_resume() -> i32 {
    let _ = std::fs::remove_file(pause_flag());
    println!("Clipboard recording resumed.");
    0
}

fn cmd_doctor() -> i32 {
    let wayland = is_wayland();
    println!("session        = {}", if wayland { "wayland" } else { "x11" });
    println!(
        "DISPLAY        = {}",
        std::env::var("DISPLAY").unwrap_or_else(|_| "(not set)".into())
    );

    let bins: &[&str] = if wayland {
        &["ydotool", "wl-copy", "gsettings"]
    } else {
        &["xdotool", "xclip", "gsettings"]
    };
    for bin in bins {
        println!(
            "{:<14} = {}",
            bin,
            which(bin)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "MISSING".into())
        );
    }

    // On Wayland auto-paste goes through ydotool, which needs the ydotoold
    // daemon reachable over its socket and access to /dev/uinput (the `input`
    // group). Check both explicitly so a broken setup is obvious.
    if wayland {
        let in_input = Command::new("id")
            .arg("-nG")
            .output()
            .ok()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .split_whitespace()
                    .any(|g| g == "input")
            })
            .unwrap_or(false);
        println!(
            "input group    = {}",
            if in_input {
                "yes"
            } else {
                "NO - run install.sh, then log out and back in"
            }
        );

        let sock = std::env::var("YDOTOOL_SOCKET").unwrap_or_else(|_| {
            let rt = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
            format!("{rt}/.ydotool_socket")
        });
        let status = if std::path::Path::new(&sock).exists() {
            format!("running (socket: {sock})")
        } else {
            format!("NOT running (no socket at {sock}) - `systemctl --user start ydotool` or re-login")
        };
        println!("ydotoold       = {status}");
    }
    println!(
        "daemon         = {}",
        if ipc::daemon_alive() {
            "running"
        } else {
            "NOT running (xclip will hold the clipboard)"
        }
    );
    match clip::Clip::new() {
        Ok(c) => {
            println!(
                "X11 clipboard  = OK (TIMESTAMP: {})",
                if c.signature().is_some() {
                    "yes"
                } else {
                    "no, using the fallback"
                }
            );
            let names = c.target_names();
            println!(
                "TARGETS        = {}",
                if names.is_empty() {
                    "(none / clipboard empty)".to_string()
                } else {
                    names.join(" ")
                }
            );
            match c.grab() {
                clip::Grab::Text(t) => println!("current content = text ({} bytes)", t.len()),
                clip::Grab::Image(d, m) => {
                    println!("current content = {} ({})", m, human_bytes(d.len()))
                }
                clip::Grab::Empty => println!("current content = (empty)"),
            }
        }
        Err(e) => println!("X11 clipboard  = ERROR: {e}"),
    }
    match db::open() {
        Ok(con) => {
            let t: i64 = con
                .query_row("SELECT COUNT(*) FROM entries WHERE kind='text'", [], |r| {
                    r.get(0)
                })
                .unwrap_or(0);
            let i: i64 = con
                .query_row("SELECT COUNT(*) FROM entries WHERE kind='image'", [], |r| {
                    r.get(0)
                })
                .unwrap_or(0);
            let pinned: i64 = con
                .query_row("SELECT COUNT(*) FROM entries WHERE pinned=1", [], |r| r.get(0))
                .unwrap_or(0);
            println!(
                "DB             = {} ({} text, {} images, {} pinned)",
                db_path().display(),
                t,
                i,
                pinned
            );
            println!(
                "max-entries    = {} (pinned excluded) | blur-close = {} | theme = {}",
                db::max_entries(),
                if setting_bool("blur_close", true) { "on" } else { "off" },
                setting("theme").unwrap_or_else(|| "light".into())
            );
            println!(
                "opacity        = {}",
                setting("opacity").unwrap_or_else(|| "0.88".into())
            );
            println!(
                "blur-close     = {}",
                if setting_bool("blur_close", true) { "on" } else { "off" }
            );
        }
        Err(e) => println!("DB             = ERROR: {e}"),
    }
    0
}

fn autostart_path() -> std::path::PathBuf {
    home().join(".config/autostart/cliphist-lite.desktop")
}

fn cmd_install(args: &[String]) -> i32 {
    let _ = ensure_dirs();
    let hotkey = flag_value(args, "--hotkey").unwrap_or_else(|| "<Super>v".to_string());
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("could not determine the binary path: {e}");
            return 1;
        }
    };

    if let Some(parent) = autostart_path().parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName=Clipboard History Daemon\n\
         Exec={} daemon\nX-GNOME-Autostart-enabled=true\nNoDisplay=true\n",
        exe.display()
    );
    if let Err(e) = std::fs::write(autostart_path(), desktop) {
        eprintln!("could not write the autostart file: {e}");
        return 1;
    }
    println!("Autostart: {}", autostart_path().display());

    if which("gsettings").is_some() {
        install_hotkey(&exe, &hotkey);
    } else {
        eprintln!("[{APP}] gsettings not found - bind the hotkey yourself in Settings > Keyboard.");
    }
    println!("\nStart the daemon now:");
    println!(
        "  nohup {} daemon >{}/daemon.log 2>&1 &",
        exe.display(),
        data_dir().display()
    );
    0
}

fn gsettings_get(schema: &str, key: &str, path: Option<&str>) -> String {
    let target = match path {
        Some(p) => format!("{}:{}", schema, p),
        None => schema.to_string(),
    };
    Command::new("gsettings")
        .args(["get", &target, key])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn gsettings_set(schema: &str, key: &str, value: &str, path: Option<&str>) {
    let target = match path {
        Some(p) => format!("{}:{}", schema, p),
        None => schema.to_string(),
    };
    let _ = Command::new("gsettings")
        .args(["set", &target, key, value])
        .status();
}

fn install_hotkey(exe: &std::path::Path, hotkey: &str) {
    const SCHEMA: &str = "org.cinnamon.desktop.keybindings";
    let raw = gsettings_get(SCHEMA, "custom-list", None);
    let mut existing: Vec<String> = raw
        .trim_start_matches("@as")
        .trim()
        .trim_matches(|c| c == '[' || c == ']')
        .split(',')
        .map(|s| s.trim().trim_matches('\'').to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let mut slot = String::from("custom0");
    let mut i = 0;
    while existing.contains(&slot) {
        let p = format!("/org/cinnamon/desktop/keybindings/custom-keybindings/{}/", slot);
        let cmdline = gsettings_get(&format!("{}.custom-keybinding", SCHEMA), "command", Some(&p));
        if cmdline.contains("cliphist") {
            break;
        }
        i += 1;
        slot = format!("custom{}", i);
    }

    let path = format!("/org/cinnamon/desktop/keybindings/custom-keybindings/{}/", slot);
    let kb = format!("{}.custom-keybinding", SCHEMA);
    gsettings_set(&kb, "name", "'Clipboard History'", Some(&path));
    gsettings_set(
        &kb,
        "command",
        &format!("'{} pick --paste'", exe.display()),
        Some(&path),
    );
    gsettings_set(&kb, "binding", &format!("['{}']", hotkey), Some(&path));

    if !existing.contains(&slot) {
        existing.push(slot.clone());
    }
    let list = format!(
        "[{}]",
        existing
            .iter()
            .map(|s| format!("'{}'", s))
            .collect::<Vec<_>>()
            .join(", ")
    );
    gsettings_set(SCHEMA, "custom-list", &list, None);
    println!("Hotkey {} -> {}", hotkey, slot);
}

fn cmd_uninstall() -> i32 {
    let _ = std::fs::remove_file(autostart_path());
    println!("Autostart removed. Delete the hotkey in Settings > Keyboard > Shortcuts.");
    println!(
        "  Data is still in {} (remove with: rm -rf {})",
        data_dir().display(),
        data_dir().display()
    );
    0
}
