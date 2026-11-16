//! A tiny HTTP server (std only, no async runtime). It serves the overlay
//! page and pushes subtitle events to it with Server-Sent Events.

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const OVERLAY_HTML: &str = include_str!("../web/overlay.html");
const SETUP_HTML: &str = include_str!("../web/setup.html");

/// Fans subtitle events out to every connected overlay.
#[derive(Default)]
pub struct Hub {
    clients: Mutex<Vec<Sender<Arc<str>>>>,
}

impl Hub {
    pub fn send(&self, msg: &str) {
        let msg: Arc<str> = msg.into();
        let mut clients = self.clients.lock().unwrap_or_else(|e| e.into_inner());
        clients.retain(|c| c.send(msg.clone()).is_ok());
    }

    fn subscribe(&self) -> mpsc::Receiver<Arc<str>> {
        let (tx, rx) = mpsc::channel();
        self.clients
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(tx);
        rx
    }
}

/// Starts listening and returns the base URL to show the user.
pub fn start(host: &str, port: u16, hub: Arc<Hub>) -> Result<String, String> {
    let listener = TcpListener::bind((host, port)).map_err(|e| {
        format!("Could not use port {port} on {host} ({e}). Is the app already running? Try --port <another number>.")
    })?;

    // Only accept requests addressed to this machine by name, which blocks
    // DNS-rebinding tricks from web pages. Skipped when serving the LAN.
    let local_only = matches!(host, "127.0.0.1" | "localhost" | "::1");
    let allowed: Arc<[String]> = [
        format!("127.0.0.1:{port}"),
        format!("localhost:{port}"),
        format!("[::1]:{port}"),
    ]
    .into();

    thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            let hub = hub.clone();
            let allowed = allowed.clone();
            thread::spawn(move || {
                let _ = handle(conn, &hub, local_only.then_some(&allowed[..]));
            });
        }
    });

    let shown = if matches!(host, "0.0.0.0" | "::") {
        "127.0.0.1"
    } else {
        host
    };
    Ok(format!("http://{shown}:{port}/"))
}

fn handle(mut s: TcpStream, hub: &Hub, allowed_hosts: Option<&[String]>) -> io::Result<()> {
    s.set_read_timeout(Some(Duration::from_secs(5)))?;
    s.set_nodelay(true)?;

    let mut buf = [0u8; 8192];
    let mut len = 0;
    while !buf[..len].windows(4).any(|w| w == b"\r\n\r\n") {
        if len == buf.len() {
            return respond(
                &mut s,
                "431 Request Header Fields Too Large",
                "text/plain",
                b"",
            );
        }
        let n = s.read(&mut buf[len..])?;
        if n == 0 {
            return Ok(());
        }
        len += n;
    }

    let req = String::from_utf8_lossy(&buf[..len]);
    let mut lines = req.lines();
    let mut first = lines.next().unwrap_or_default().split_whitespace();
    let method = first.next().unwrap_or_default();
    let path = first.next().unwrap_or("/").split('?').next().unwrap_or("/");

    if let Some(allowed) = allowed_hosts {
        let host = lines
            .find_map(|l| {
                l.split_once(':')
                    .filter(|(k, _)| k.eq_ignore_ascii_case("host"))
            })
            .map(|(_, v)| v.trim())
            .unwrap_or_default();
        if !allowed.iter().any(|a| a.eq_ignore_ascii_case(host)) {
            return respond(&mut s, "403 Forbidden", "text/plain", b"Forbidden");
        }
    }

    if method != "GET" {
        return respond(&mut s, "405 Method Not Allowed", "text/plain", b"");
    }

    match path {
        "/" | "/setup" => respond(
            &mut s,
            "200 OK",
            "text/html; charset=utf-8",
            SETUP_HTML.as_bytes(),
        ),
        "/overlay" => respond(
            &mut s,
            "200 OK",
            "text/html; charset=utf-8",
            OVERLAY_HTML.as_bytes(),
        ),
        "/events" => events(s, hub),
        "/favicon.ico" => respond(&mut s, "204 No Content", "text/plain", b""),
        _ => respond(&mut s, "404 Not Found", "text/plain", b"Not found"),
    }
}

fn respond(s: &mut TcpStream, status: &str, kind: &str, body: &[u8]) -> io::Result<()> {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    s.write_all(head.as_bytes())?;
    s.write_all(body)?;
    s.flush()
}

/// Server-Sent Events stream: one `data:` line per subtitle update.
fn events(mut s: TcpStream, hub: &Hub) -> io::Result<()> {
    s.set_read_timeout(None)?;
    s.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\n\
          Connection: keep-alive\r\n\r\nretry: 1000\n\n",
    )?;
    let rx = hub.subscribe();
    loop {
        match rx.recv_timeout(Duration::from_secs(15)) {
            Ok(msg) => s.write_all(format!("data: {msg}\n\n").as_bytes())?,
            // A comment line keeps idle connections open and detects closed ones.
            Err(RecvTimeoutError::Timeout) => s.write_all(b": ping\n\n")?,
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}
