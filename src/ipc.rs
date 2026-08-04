use crate::util::socket_path;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc::{channel, Sender};
use std::time::{Duration, Instant};

/// The picker gives up after this long. The daemon uses the same value to drop
/// requests it can no longer answer in time, so a late reply never applies a
/// clipboard change the picker has already handled by itself.
pub const REQUEST_DEADLINE: Duration = Duration::from_secs(9);

/// A request sent from the picker to the daemon.
pub enum Req {
    /// Put entry `id` on the clipboard; the reply goes back through Sender.
    /// The Instant records when the picker asked, so the daemon can tell
    /// whether the request is still worth applying.
    Set(i64, Instant, Sender<String>),
}

/// Bind the socket and listen on a dedicated thread.
pub fn serve(tx: Sender<Req>) -> std::io::Result<()> {
    let path = socket_path();
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;
    crate::util::chmod(&path, 0o600);

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let tx = tx.clone();
            std::thread::spawn(move || {
                let _ = stream.set_read_timeout(Some(REQUEST_DEADLINE + Duration::from_secs(1)));
                let mut line = String::new();
                let mut reader = BufReader::new(match stream.try_clone() {
                    Ok(s) => s,
                    Err(_) => return,
                });
                if reader.read_line(&mut line).is_err() {
                    return;
                }
                let reply = handle(line.trim(), &tx);
                let _ = writeln!(stream, "{}", reply);
                let _ = stream.flush();
            });
        }
    });
    Ok(())
}

fn handle(line: &str, tx: &Sender<Req>) -> String {
    let mut parts = line.splitn(2, ' ');
    match (parts.next(), parts.next()) {
        (Some("PING"), _) => "OK".into(),
        (Some("SET"), Some(arg)) => match arg.trim().parse::<i64>() {
            Ok(id) => {
                let (rtx, rrx) = channel();
                if tx.send(Req::Set(id, Instant::now(), rtx)).is_err() {
                    return "ERR daemon busy".into();
                }
                rrx.recv_timeout(REQUEST_DEADLINE)
                    .unwrap_or_else(|_| "ERR timeout".into())
            }
            Err(_) => "ERR invalid id".into(),
        },
        _ => "ERR unknown command".into(),
    }
}

/// Picker side: ask the daemon to set the clipboard. Err means it is not ready.
pub fn client_set(id: i64) -> Result<(), String> {
    let mut stream = UnixStream::connect(socket_path()).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(REQUEST_DEADLINE + Duration::from_secs(1)))
        .map_err(|e| e.to_string())?;
    writeln!(stream, "SET {}", id).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new(&stream)
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    if line.trim() == "OK" {
        Ok(())
    } else {
        Err(line.trim().to_string())
    }
}

pub fn daemon_alive() -> bool {
    UnixStream::connect(socket_path())
        .map(|mut s| {
            let _ = s.set_read_timeout(Some(Duration::from_millis(500)));
            let _ = writeln!(s, "PING");
            let mut line = String::new();
            BufReader::new(&s).read_line(&mut line).is_ok() && line.trim() == "OK"
        })
        .unwrap_or(false)
}
