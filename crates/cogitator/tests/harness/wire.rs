//! Content-Length framing, shared by every harness send/receive path
//! (one implementation instead of the three copies the v1 harness grew).

use std::io::{BufRead, Write};

/// Encode one LSP frame onto `w`.
pub fn write_frame(w: &mut impl Write, msg: &serde_json::Value) {
    let body = serde_json::to_vec(msg).expect("frame serializes");
    write!(w, "Content-Length: {}\r\n\r\n", body.len()).expect("frame header writes");
    w.write_all(&body).expect("frame body writes");
    w.flush().expect("frame flushes");
}

/// Decode one LSP frame from `r`.
///
/// Errors are returned (not panicked) because this runs on the reader
/// thread — the test thread turns them into panics WITH the server's
/// stderr attached. The stdout-cleanliness guard lives here: nothing but
/// LSP frames may appear on stdout (05 §6.3), so the first header line
/// must be a Content-Length.
pub fn read_frame(r: &mut impl BufRead) -> Result<serde_json::Value, String> {
    let mut line = String::new();
    let mut content_length: Option<usize> = None;
    let mut first = true;
    loop {
        line.clear();
        let n = r
            .read_line(&mut line)
            .map_err(|e| format!("stdout read: {e}"))?;
        if n == 0 {
            return Err(if first {
                "server closed stdout".into()
            } else {
                format!("server closed stdout mid-header (after {line:?})")
            });
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if first {
            if !trimmed.starts_with("Content-Length:") {
                return Err(format!("non-LSP bytes on stdout: {trimmed:?}"));
            }
            first = false;
        }
        if trimmed.is_empty() {
            break; // end of headers
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = value.trim().parse().ok();
        }
    }
    let len = content_length.ok_or("header without Content-Length")?;
    let mut body = vec![0u8; len];
    std::io::Read::read_exact(r, &mut body).map_err(|e| format!("stdout body read: {e}"))?;
    serde_json::from_slice(&body).map_err(|e| format!("frame is not JSON: {e}"))
}
