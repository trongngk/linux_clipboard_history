use crate::clip::{Clip, Grab};
use crate::db;
use crate::ipc::{self, Req};
use crate::util::*;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::Duration;

/// Owners that never answer TIMESTAMP (xclip, some terminals) keep the same
/// change token across copies. Rescan this often so their copies still land.
const FULL_CHECK_EVERY: u64 = 5;
/// Catch-all rescan for the rarer owners that reassert the selection with an
/// *unchanged* TIMESTAMP: the token check alone would never notice those.
const SAFETY_RESCAN_EVERY: u64 = 30;
/// A slow owner (Java apps, large INCR payloads) can answer the first request
/// with nothing. Retry this many ticks before accepting that it is empty, so a
/// slow read is not silently dropped.
const READ_RETRIES: u32 = 6;

pub fn run(interval_ms: u64) -> Result<(), String> {
    let _lock = match acquire_lock(&daemon_lock()).map_err(|e| e.to_string())? {
        LockResult::Acquired(l) => l,
        LockResult::Held(pid) => {
            return Err(match pid {
                Some(p) => format!("daemon is already running (pid {p}). Stop it with: kill {p}"),
                None => "daemon is already running".into(),
            })
        }
    };

    let clip = Clip::new()?;
    let con = db::open().map_err(|e| e.to_string())?;
    let mut rules = db::load_ignore_rules();
    let mut rules_mtime = mtime(&ignore_file());

    let (tx, rx) = channel::<Req>();
    ipc::serve(tx).map_err(|e| format!("failed to bind socket: {e}"))?;
    eprintln!(
        "[{APP}] daemon started (interval={}ms, db={}, sock={})",
        interval_ms,
        db_path().display(),
        socket_path().display()
    );

    let mut last_token: (Option<Vec<u8>>, Option<u32>) = (None, None);
    let mut tick: u64 = 0;
    let mut miss: u32 = 0;

    capture(&clip, &con, &rules);

    loop {
        // Wait for the next poll tick, but wake immediately on a SET request
        match rx.recv_timeout(Duration::from_millis(interval_ms)) {
            Ok(req) => {
                handle(req, &clip, &con, &mut last_token);
                // Drain anything else that queued up while we were reading the
                // clipboard, so a backlog cannot apply an old entry later.
                drain(&rx, &clip, &con, &mut last_token);
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => {}
            Err(RecvTimeoutError::Timeout) => {}
        }

        tick += 1;
        if pause_flag().exists() {
            continue;
        }

        let mt = mtime(&ignore_file());
        if mt != rules_mtime {
            rules = db::load_ignore_rules();
            rules_mtime = mt;
        }

        let token = clip.change_token();
        if token.1.is_none() {
            continue; // nobody owns the clipboard
        }

        if token != last_token {
            // Clipboard changed. Only remember this token once we have actually
            // read content off it: a slow owner can answer the first request
            // with nothing, and advancing the token now would drop that entry
            // for good. Keep retrying for a bounded number of ticks instead.
            if capture(&clip, &con, &rules) {
                last_token = token;
                miss = 0;
            } else {
                miss += 1;
                if miss >= READ_RETRIES {
                    last_token = token; // give up: the owner is genuinely empty
                    miss = 0;
                }
            }
        } else if (last_token.0.is_none() && tick % FULL_CHECK_EVERY == 0)
            || tick % SAFETY_RESCAN_EVERY == 0
        {
            // Token unchanged but the content may not be (see the constants
            // above). Identical content is deduped by hash, so a rescan only
            // bumps ts and is safe to run.
            capture(&clip, &con, &rules);
        }
    }

    #[allow(unreachable_code)]
    {
        drop(_lock);
        Ok(())
    }
}

/// Apply one picker request. Requests older than the picker's own deadline are
/// dropped: it has already fallen back to xclip, so setting the clipboard now
/// would overwrite whatever the user copied in the meantime.
fn handle(
    req: Req,
    clip: &Clip,
    con: &rusqlite::Connection,
    last_token: &mut (Option<Vec<u8>>, Option<u32>),
) {
    let Req::Set(id, asked_at, reply) = req;
    if asked_at.elapsed() > crate::ipc::REQUEST_DEADLINE {
        let _ = reply.send("ERR stale request, dropped".to_string());
        eprintln!("[{APP}] dropped a stale SET for id={id}");
        return;
    }
    let msg = match db::get(con, id) {
        Ok(Some(e)) => match clip.put(&e) {
            Ok(()) => {
                // We own the selection now; the next pass reads it back and
                // hash dedupe floats that entry to the top
                *last_token = (None, None);
                "OK".to_string()
            }
            Err(e) => format!("ERR {e}"),
        },
        Ok(None) => format!("ERR no entry with id={id}"),
        Err(e) => format!("ERR {e}"),
    };
    let _ = reply.send(msg);
}

fn drain(
    rx: &Receiver<Req>,
    clip: &Clip,
    con: &rusqlite::Connection,
    last_token: &mut (Option<Vec<u8>>, Option<u32>),
) {
    while let Ok(req) = rx.try_recv() {
        handle(req, clip, con, last_token);
    }
}

/// Read the current clipboard and store it. Returns `true` when something was
/// actually read off the clipboard (text or image), `false` when the read came
/// back empty - which may just mean a slow owner that was not ready yet.
fn capture(clip: &Clip, con: &rusqlite::Connection, rules: &[regex::Regex]) -> bool {
    match clip.grab() {
        Grab::Text(t) => {
            if let Err(e) = db::store_text(con, &t, rules) {
                eprintln!("[{APP}] failed to store text: {e}");
            }
            true
        }
        Grab::Image(data, mime) => {
            if let Err(e) = db::store_image(con, &data, mime) {
                eprintln!("[{APP}] failed to store image: {e}");
            }
            true
        }
        Grab::Empty => false,
    }
}

fn mtime(p: &std::path::Path) -> u64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
