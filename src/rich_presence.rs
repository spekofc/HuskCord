// language: Rust, file: src/rich_presence.rs
//
// Discord Rich Presence over the local IPC pipe.
//
// Discord listens on \\.\pipe\discord-ipc-0 .. discord-ipc-9. Protocol is
// length-prefixed JSON frames:
//
//   [u32 opcode LE][u32 length LE][utf8 json of <length> bytes]
//
// opcodes:
//   0 handshake  {"v":1,"client_id":"<app id>"}
//   1 frame      {"cmd":"SET_ACTIVITY","nonce":..,"args":{"pid":..,"activity":{..}}}
//   2 close
//   3 ping       {"nonce":..}          -> reply opcode 4 (pong)
//
// A running Discord desktop client answers whichever pipe is free. Our own
// shell does NOT open the pipe, so a user running both gets presence through
// the real client. This module only implements the client side of the protocol.

use std::io::{Read, Write};

const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_PING: u32 = 3;
const OP_PONG: u32 = 4;

fn pipe_path(i: u8) -> String {
    format!(r"\\.\pipe\discord-ipc-{i}")
}

/// Frame one message: 4-byte opcode, 4-byte length, then the JSON bytes.
fn frame(op: u32, json: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8 + json.len());
    buf.extend_from_slice(&op.to_le_bytes());
    buf.extend_from_slice(&(json.len() as u32).to_le_bytes());
    buf.extend_from_slice(json.as_bytes());
    buf
}

pub struct Presence {
    stream: std::fs::File,
    nonce: u64,
}

impl Presence {
    /// Connect and handshake. `client_id` is a Discord application id.
    pub fn connect(client_id: &str) -> Option<Presence> {
        for i in 0..10u8 {
            let path = pipe_path(i);
            let stream = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .ok();

            if let Some(mut s) = stream {
                let hs = format!(r#"{{"v":1,"client_id":"{client_id}"}}"#);
                if s.write_all(&frame(OP_HANDSHAKE, &hs)).is_err() {
                    continue;
                }
                return Some(Presence { stream: s, nonce: 1 });
            }
        }
        None
    }

    /// Send a SET_ACTIVITY frame. Fields left empty are omitted.
    pub fn set_activity(&mut self, details: &str, state: &str, large_text: &str) -> bool {
        let pid = std::process::id();
        let mut activity = String::from("{");
        let mut first = true;
        if !details.is_empty() {
            activity.push_str(&format!(r#""details":"{}""#, escape(details)));
            first = false;
        }
        if !state.is_empty() {
            if !first {
                activity.push(',');
            }
            activity.push_str(&format!(r#""state":"{}""#, escape(state)));
            first = false;
        }
        if !large_text.is_empty() {
            if !first {
                activity.push(',');
            }
            activity.push_str(&format!(
                r#""assets":{{"large_image":"logo","large_text":"{}"}}"#,
                escape(large_text)
            ));
        }
        activity.push_str(&format!(r#","timestamps":{{"start":{}}}"#, now_unix()));
        activity.push('}');

        let payload = format!(
            r#"{{"cmd":"SET_ACTIVITY","nonce":"{}","args":{{"pid":{},"activity":{}}}}}"#,
            self.nonce, pid, activity
        );
        self.nonce += 1;

        self.stream.write_all(&frame(OP_FRAME, &payload)).is_ok()
    }

    /// Drain anything Discord sent (pongs, errors). Non-blocking by caller.
    pub fn pump(&mut self) {
        let mut head = [0u8; 8];
        if self.stream.read_exact(&mut head).is_err() {
            return;
        }
        let op = u32::from_le_bytes([head[0], head[1], head[2], head[3]]);
        let len = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize;
        if len > 0 && len < 65_536 {
            let mut body = vec![0u8; len];
            let _ = self.stream.read_exact(&mut body);
        }
        if op == OP_PING {
            let _ = self.stream.write_all(&frame(OP_PONG, "{}"));
        }
    }
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
