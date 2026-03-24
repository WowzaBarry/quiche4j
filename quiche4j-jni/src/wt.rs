//! WebTransport server implementation on top of quiche h3.
//!
//! quiche's h3 layer classifies WT stream type prefixes (0x41, 0x54) as
//! `Type::Unknown`. This module intercepts readable streams before h3 sees
//! them, parses WT prefixes, and maintains session→stream routing.

use quiche::{h3, Connection};
use quiche::h3::NameValue;
use std::collections::HashMap;

/// WebTransport stream type prefix bytes (variable-length integers).
/// 0x41 = bidirectional WT stream, 0x54 = unidirectional WT stream.
const WT_STREAM_TYPE_BIDI: u64 = 0x41;
const WT_STREAM_TYPE_UNI: u64 = 0x54;

/// Events produced by polling the WtServer.
pub enum WtEvent {
    /// New WebTransport session established via Extended CONNECT.
    NewSession {
        session_id: u64,
        path: String,
    },
    /// New WT data stream opened by peer.
    NewStream {
        session_id: u64,
        stream_id: u64,
        bidi: bool,
    },
    /// Data available on a WT stream. Caller should read via conn.stream_recv().
    Data {
        stream_id: u64,
    },
    /// A WT stream was finished (FIN received).
    Finished {
        stream_id: u64,
    },
    /// An H3 event that is not WT-related (forwarded from h3::Connection::poll).
    H3Headers {
        stream_id: u64,
        headers: Vec<h3::Header>,
        has_body: bool,
    },
    H3Data {
        stream_id: u64,
    },
    H3Finished {
        stream_id: u64,
    },
}

/// State for a single WT stream being parsed.
enum StreamParseState {
    /// Waiting for the stream type prefix byte(s).
    AwaitingType,
    /// Type parsed, waiting for session ID varint.
    AwaitingSessionId { bidi: bool },
    /// Fully classified WT stream.
    Ready { session_id: u64, bidi: bool },
}

/// WebTransport server wrapping an h3::Connection.
pub struct WtServer {
    /// Known WT streams: stream_id → parse state.
    streams: HashMap<u64, StreamParseState>,
    /// Session ID → CONNECT stream ID (the H3 request stream).
    sessions: HashMap<u64, u64>,
    /// Partial read buffers for streams still being classified.
    parse_bufs: HashMap<u64, Vec<u8>>,
}

impl WtServer {
    pub fn new() -> Self {
        WtServer {
            streams: HashMap::new(),
            sessions: HashMap::new(),
            parse_bufs: HashMap::new(),
        }
    }

    /// Poll for WebTransport events.
    ///
    /// 1. Check all readable QUIC streams. For streams not yet known to h3
    ///    (potential WT streams), try to parse the WT prefix.
    /// 2. Then poll h3 for standard HTTP/3 events (CONNECT requests, etc.).
    /// 3. Return batched events.
    pub fn poll(
        &mut self,
        conn: &mut Connection,
        h3_conn: &mut h3::Connection,
    ) -> Vec<WtEvent> {
        let mut events = Vec::new();

        // Phase 1: Classify readable streams
        let readable: Vec<u64> = conn.readable().collect();
        for stream_id in readable {
            // Skip streams managed by h3 (control streams, QPACK, request streams)
            // Client-initiated bidi streams with IDs 0,4,8... could be h3 request streams.
            // Client-initiated uni streams with IDs 2,6,10... could be h3 control/QPACK.
            // We only intercept streams that we've started tracking as WT,
            // or new client-initiated streams that might have a WT prefix.
            if let Some(state) = self.streams.get(&stream_id) {
                match state {
                    StreamParseState::Ready { .. } => {
                        // Already classified — just report data available
                        events.push(WtEvent::Data { stream_id });
                    }
                    _ => {
                        // Still parsing prefix — try to advance
                        if let Some(evt) = self.try_parse_prefix(conn, stream_id) {
                            events.push(evt);
                        }
                    }
                }
                continue;
            }

            // New stream we haven't seen. Check if it's a potential WT stream.
            // Client-initiated unidirectional streams (ID % 4 == 2) that aren't
            // the first few (h3 control, QPACK encoder/decoder) might be WT.
            // Client-initiated bidi streams (ID % 4 == 0) beyond stream 0 might be WT.
            //
            // We peek the first bytes to check for WT type prefix.
            // If it's not a WT prefix, we leave it for h3 to handle.
            if self.might_be_wt_stream(stream_id) {
                self.streams.insert(stream_id, StreamParseState::AwaitingType);
                self.parse_bufs.insert(stream_id, Vec::new());
                if let Some(evt) = self.try_parse_prefix(conn, stream_id) {
                    events.push(evt);
                }
            }
            // Otherwise, let h3 handle it in phase 2
        }

        // Phase 2: Poll h3 for standard events
        loop {
            match h3_conn.poll(conn) {
                Ok((stream_id, h3::Event::Headers { list, more_frames })) => {
                    // Check if this is an Extended CONNECT for WebTransport
                    if self.is_webtransport_connect(&list) {
                        let path = self.extract_path(&list);
                        // The session ID = the request stream ID
                        let session_id = stream_id;
                        self.sessions.insert(session_id, stream_id);

                        // Send 200 OK response to establish the WT session
                        let resp_headers = vec![
                            h3::Header::new(b":status", b"200"),
                            h3::Header::new(b"sec-webtransport-http3-draft", b"draft02"),
                        ];
                        let _ = h3_conn.send_response(conn, stream_id, &resp_headers, false);

                        events.push(WtEvent::NewSession {
                            session_id,
                            path,
                        });
                    } else {
                        events.push(WtEvent::H3Headers {
                            stream_id,
                            headers: list,
                            has_body: more_frames,
                        });
                    }
                }
                Ok((stream_id, h3::Event::Data)) => {
                    events.push(WtEvent::H3Data { stream_id });
                }
                Ok((stream_id, h3::Event::Finished)) => {
                    // Check if this is a WT session being closed
                    if self.sessions.contains_key(&stream_id) {
                        // Session's CONNECT stream finished — session ends
                        self.sessions.remove(&stream_id);
                    }
                    events.push(WtEvent::H3Finished { stream_id });
                }
                Ok((_stream_id, h3::Event::Reset(_))) |
                Ok((_stream_id, h3::Event::PriorityUpdate)) |
                Ok((_stream_id, h3::Event::GoAway)) => {
                    // Ignore these for now
                }
                Err(h3::Error::Done) => break,
                Err(_e) => break,
            }
        }

        // Phase 3: Check for FIN on classified WT streams
        for (&stream_id, state) in &self.streams {
            if let StreamParseState::Ready { .. } = state {
                if conn.stream_finished(stream_id) {
                    events.push(WtEvent::Finished { stream_id });
                }
            }
        }

        events
    }

    /// Open a server-initiated WT unidirectional stream for the given session.
    /// Writes the 0x54 type prefix + session_id varint.
    /// Returns the QUIC stream ID, or an error.
    pub fn open_uni_stream(
        &mut self,
        conn: &mut Connection,
        session_id: u64,
    ) -> Result<u64, String> {
        // Server-initiated unidirectional stream IDs: 3, 7, 11, ...
        // quiche manages stream ID allocation via stream_send on a new ID.
        // We need to pick the next available server-initiated uni stream ID.
        // quiche will auto-create the stream on first send.
        let stream_id = self.next_server_uni_stream_id(conn);

        // Build prefix: varint(0x54) + varint(session_id)
        let mut prefix = Vec::with_capacity(16);
        encode_varint(&mut prefix, WT_STREAM_TYPE_UNI);
        encode_varint(&mut prefix, session_id);

        match conn.stream_send(stream_id, &prefix, false) {
            Ok(_) => {
                self.streams.insert(
                    stream_id,
                    StreamParseState::Ready { session_id, bidi: false },
                );
                Ok(stream_id)
            }
            Err(e) => Err(format!("stream_send failed: {:?}", e)),
        }
    }

    /// Open a server-initiated WT bidirectional stream for the given session.
    /// Writes the 0x41 type prefix + session_id varint.
    pub fn open_bidi_stream(
        &mut self,
        conn: &mut Connection,
        session_id: u64,
    ) -> Result<u64, String> {
        let stream_id = self.next_server_bidi_stream_id(conn);

        let mut prefix = Vec::with_capacity(16);
        encode_varint(&mut prefix, WT_STREAM_TYPE_BIDI);
        encode_varint(&mut prefix, session_id);

        match conn.stream_send(stream_id, &prefix, false) {
            Ok(_) => {
                self.streams.insert(
                    stream_id,
                    StreamParseState::Ready { session_id, bidi: true },
                );
                Ok(stream_id)
            }
            Err(e) => Err(format!("stream_send failed: {:?}", e)),
        }
    }

    /// Get the session ID for a classified WT stream.
    pub fn stream_session_id(&self, stream_id: u64) -> Option<u64> {
        match self.streams.get(&stream_id) {
            Some(StreamParseState::Ready { session_id, .. }) => Some(*session_id),
            _ => None,
        }
    }

    /// Remove a stream from tracking (e.g., on close).
    pub fn remove_stream(&mut self, stream_id: u64) {
        self.streams.remove(&stream_id);
        self.parse_bufs.remove(&stream_id);
    }

    // --- Internal helpers ---

    /// Heuristic: could this stream be a WebTransport data stream?
    /// We consider client-initiated streams beyond the first few h3 streams.
    fn might_be_wt_stream(&self, stream_id: u64) -> bool {
        // Client-initiated bidi: ID % 4 == 0, skip stream 0 (h3 control)
        // Client-initiated uni: ID % 4 == 2, skip streams 2,6,10 (h3 control, QPACK)
        let is_client_bidi = stream_id % 4 == 0 && stream_id > 0;
        let is_client_uni = stream_id % 4 == 2 && stream_id > 10;
        is_client_bidi || is_client_uni
    }

    /// Try to read and parse the WT stream type prefix from a stream.
    fn try_parse_prefix(
        &mut self,
        conn: &mut Connection,
        stream_id: u64,
    ) -> Option<WtEvent> {
        // Read available bytes
        let mut tmp = [0u8; 32];
        loop {
            match conn.stream_recv(stream_id, &mut tmp) {
                Ok((n, _fin)) => {
                    if n == 0 { break; }
                    if let Some(buf) = self.parse_bufs.get_mut(&stream_id) {
                        buf.extend_from_slice(&tmp[..n]);
                    }
                }
                Err(_) => break,
            }
        }

        let buf = self.parse_bufs.get(&stream_id)?;
        if buf.is_empty() {
            return None;
        }

        let state = self.streams.get(&stream_id)?;
        match state {
            StreamParseState::AwaitingType => {
                // Try to decode varint for stream type
                if let Some((type_val, consumed)) = decode_varint(buf) {
                    let bidi = match type_val {
                        WT_STREAM_TYPE_BIDI => true,
                        WT_STREAM_TYPE_UNI => false,
                        _ => {
                            // Not a WT stream — remove from tracking
                            // The bytes are already consumed, so h3 can't recover them.
                            // This is the risk noted in the plan.
                            self.streams.remove(&stream_id);
                            self.parse_bufs.remove(&stream_id);
                            return None;
                        }
                    };

                    // Advance buffer past the type varint
                    let remaining = buf[consumed..].to_vec();
                    self.parse_bufs.insert(stream_id, remaining.clone());
                    self.streams.insert(stream_id, StreamParseState::AwaitingSessionId { bidi });

                    // Try to also parse the session ID
                    if let Some((session_id, consumed2)) = decode_varint(&remaining) {
                        let leftover = remaining[consumed2..].to_vec();
                        self.parse_bufs.insert(stream_id, leftover);
                        self.streams.insert(
                            stream_id,
                            StreamParseState::Ready { session_id, bidi },
                        );
                        return Some(WtEvent::NewStream {
                            session_id,
                            stream_id,
                            bidi,
                        });
                    }
                }
                None
            }
            StreamParseState::AwaitingSessionId { bidi } => {
                let bidi = *bidi;
                if let Some((session_id, consumed)) = decode_varint(buf) {
                    let leftover = buf[consumed..].to_vec();
                    self.parse_bufs.insert(stream_id, leftover);
                    self.streams.insert(
                        stream_id,
                        StreamParseState::Ready { session_id, bidi },
                    );
                    return Some(WtEvent::NewStream {
                        session_id,
                        stream_id,
                        bidi,
                    });
                }
                None
            }
            StreamParseState::Ready { .. } => {
                // Already classified, shouldn't be called
                None
            }
        }
    }

    fn is_webtransport_connect(&self, headers: &[h3::Header]) -> bool {
        let mut is_connect = false;
        let mut has_protocol = false;
        for h in headers {
            let name = std::str::from_utf8(h.name()).unwrap_or("");
            let val = std::str::from_utf8(h.value()).unwrap_or("");
            if name == ":method" && val == "CONNECT" {
                is_connect = true;
            }
            if name == ":protocol" && val == "webtransport" {
                has_protocol = true;
            }
        }
        is_connect && has_protocol
    }

    fn extract_path(&self, headers: &[h3::Header]) -> String {
        for h in headers {
            if std::str::from_utf8(h.name()).unwrap_or("") == ":path" {
                return std::str::from_utf8(h.value()).unwrap_or("").to_string();
            }
        }
        String::new()
    }

    /// Get the next server-initiated unidirectional stream ID.
    /// Server uni streams: 3, 7, 11, 15, ... (ID % 4 == 3)
    fn next_server_uni_stream_id(&self, _conn: &Connection) -> u64 {
        let mut max_id: u64 = 3;
        for &sid in self.streams.keys() {
            if sid % 4 == 3 && sid >= max_id {
                max_id = sid + 4;
            }
        }
        max_id
    }

    /// Get the next server-initiated bidirectional stream ID.
    /// Server bidi streams: 1, 5, 9, 13, ... (ID % 4 == 1)
    fn next_server_bidi_stream_id(&self, _conn: &Connection) -> u64 {
        let mut max_id: u64 = 1;
        for &sid in self.streams.keys() {
            if sid % 4 == 1 && sid >= max_id {
                max_id = sid + 4;
            }
        }
        max_id
    }
}

// --- QUIC varint encoding/decoding (RFC 9000 §16) ---

fn decode_varint(buf: &[u8]) -> Option<(u64, usize)> {
    if buf.is_empty() {
        return None;
    }
    let first = buf[0];
    let prefix = (first >> 6) as usize;
    let len = 1 << prefix;
    if buf.len() < len {
        return None;
    }
    let mut val = (first & 0x3f) as u64;
    for i in 1..len {
        val = (val << 8) | (buf[i] as u64);
    }
    Some((val, len))
}

fn encode_varint(buf: &mut Vec<u8>, val: u64) {
    if val <= 0x3f {
        buf.push(val as u8);
    } else if val <= 0x3fff {
        buf.push(((val >> 8) as u8) | 0x40);
        buf.push(val as u8);
    } else if val <= 0x3fffffff {
        let bytes = (val as u32).to_be_bytes();
        buf.push(bytes[0] | 0x80);
        buf.extend_from_slice(&bytes[1..]);
    } else {
        let bytes = val.to_be_bytes();
        buf.push(bytes[0] | 0xc0);
        buf.extend_from_slice(&bytes[1..]);
    }
}
