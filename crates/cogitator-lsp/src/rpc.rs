//! Base-protocol framing (specs/rust-port/01 §1).
//!
//! Headers are parsed properly (fixes Q10): the header block may contain
//! several headers; `Content-Length` is matched case-insensitively.

use cogitator_error::{Error, Result};
use serde::Serialize;

/// A complete frame located inside a read buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    /// Total frame length (headers + separator + content).
    pub total_len: usize,
    /// Byte offset where the JSON content starts.
    pub content_start: usize,
    /// Content length in bytes.
    pub content_len: usize,
}

/// Try to locate one complete frame at the start of `buf`.
///
/// Returns `Ok(None)` when more bytes are needed.
pub fn parse_frame(buf: &[u8]) -> Result<Option<Frame>> {
    let Some(sep) = find_subsequence(buf, b"\r\n\r\n") else {
        return Ok(None);
    };
    let header_block = &buf[..sep];
    let mut content_len: Option<usize> = None;
    for line in header_block.split(|&b| b == b'\n') {
        let line = std::str::from_utf8(line)
            .map_err(|_| Error::Frame {
                reason: "non-utf8 header".into(),
            })?
            .trim_end_matches('\r')
            .trim();
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_len = Some(value.trim().parse::<usize>().map_err(|_| Error::Frame {
                    reason: format!("bad Content-Length: {value:?}"),
                })?);
            }
        }
    }
    let content_len = content_len.ok_or_else(|| Error::Frame {
        reason: "missing Content-Length header".into(),
    })?;
    let content_start = sep + 4;
    if buf.len() < content_start + content_len {
        return Ok(None);
    }
    Ok(Some(Frame {
        total_len: content_start + content_len,
        content_start,
        content_len,
    }))
}

/// Extract the `method` field without deserializing the whole message.
pub fn decode_method(content: &[u8]) -> Result<String> {
    let base: crate::types::BaseMessage = serde_json::from_slice(content)?;
    Ok(base.method.map(|m| m.into_owned()).unwrap_or_default())
}

/// Serialize `msg` as one framed message into `out` (08 §1.4: reused buffers,
/// no intermediate `String`). `scratch` holds the JSON body between calls.
pub fn encode_frame<T: Serialize>(msg: &T, scratch: &mut Vec<u8>, out: &mut Vec<u8>) -> Result<()> {
    scratch.clear();
    serde_json::to_writer(&mut *scratch, msg)?;
    out.clear();
    use std::io::Write;
    write!(out, "Content-Length: {}\r\n\r\n", scratch.len()).expect("writing to Vec cannot fail");
    out.extend_from_slice(scratch);
    Ok(())
}

/// Convenience for tests and one-off writes.
pub fn encode_frame_vec<T: Serialize>(msg: &T) -> Result<Vec<u8>> {
    let mut scratch = Vec::new();
    let mut out = Vec::new();
    encode_frame(msg, &mut scratch, &mut out)?;
    Ok(out)
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[derive(serde::Serialize)]
    struct Enc {
        testing: bool,
    }

    // Port of rpc/rpc_test.go
    #[test]
    fn encode_matches_go() {
        let out = encode_frame_vec(&Enc { testing: true }).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "Content-Length: 16\r\n\r\n{\"testing\":true}"
        );
    }

    #[test]
    fn decode_roundtrip() {
        let msg = b"Content-Length: 15\r\n\r\n{\"method\":\"hi\"}";
        let frame = parse_frame(msg).unwrap().unwrap();
        assert_eq!(frame.content_len, 15);
        let content = &msg[frame.content_start..frame.content_start + frame.content_len];
        assert_eq!(decode_method(content).unwrap(), "hi");
    }

    #[test]
    fn incomplete_frame_needs_more() {
        assert_eq!(
            parse_frame(b"Content-Length: 10\r\n\r\n{\"a\"").unwrap(),
            None
        );
        assert_eq!(parse_frame(b"Content-Len").unwrap(), None);
    }

    #[test]
    fn extra_headers_accepted() {
        let msg = b"Content-Length: 2\r\nContent-Type: application/vscode-jsonrpc\r\n\r\n{}";
        let frame = parse_frame(msg).unwrap().unwrap();
        assert_eq!(frame.content_len, 2);
    }
}
