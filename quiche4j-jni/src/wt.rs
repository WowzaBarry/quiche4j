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
#[derive(Clone, Copy)]
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
    /// Drained payload bytes for classified WT streams. We pull data out of
    /// quiche in Phase 1 of `poll` so quiche-h3 (Phase 2) can never consume
    /// MoQ payload bytes on a client-bidi stream — h3 would otherwise treat
    /// it as `Type::Request` / `State::FrameType` and parse our payload as
    /// HTTP/3 frames.
    stream_data: HashMap<u64, Vec<u8>>,
    /// Whether each Ready stream has reached FIN (after draining).
    /// We emit `WtEvent::Finished` exactly once per stream when its buffer
    /// drains empty after FIN was observed.
    stream_fin: HashMap<u64, bool>,
    /// Streams whose FIN was already reported via `WtEvent::Finished`,
    /// kept so we never report twice and so we can ignore stale h3 events.
    finished_reported: HashMap<u64, bool>,
}

impl WtServer {
    pub fn new() -> Self {
        WtServer {
            streams: HashMap::new(),
            sessions: HashMap::new(),
            parse_bufs: HashMap::new(),
            stream_data: HashMap::new(),
            stream_fin: HashMap::new(),
            finished_reported: HashMap::new(),
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

        // Phase 1: Classify readable streams and drain WT payload.
        //
        // Critical: we must drain bytes from any classified WT stream BEFORE
        // calling h3_conn.poll. quiche-h3 auto-classifies client-bidi streams
        // (ID % 4 == 0) as Type::Request / State::FrameType and would happily
        // call conn.stream_recv on them, eating our MoQ payload as if it were
        // an HTTP/3 frame type+length pair.
        let readable: Vec<u64> = conn.readable().collect();
        for stream_id in readable {
            if let Some(state) = self.streams.get(&stream_id).copied() {
                match state {
                    StreamParseState::Ready { .. } => {
                        // Already classified — drain into our buffer so h3.poll
                        // can't see any of these bytes.
                        if self.drain_stream(conn, stream_id) {
                            events.push(WtEvent::Data { stream_id });
                        }
                    }
                    _ => {
                        // Still parsing prefix — try to advance, then drain
                        // any payload that follows the prefix.
                        if let Some(evt) = self.try_parse_prefix(conn, stream_id) {
                            events.push(evt);
                            // After try_parse_prefix returns NewStream, the
                            // stream is now Ready. Drain leftover payload.
                            self.drain_stream(conn, stream_id);
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
                    self.drain_stream(conn, stream_id);
                }
            }
            // Otherwise, let h3 handle it in phase 2
        }

        // Phase 2: Poll h3 for standard events. Suppress events for any stream
        // we've classified as a WT data stream — h3 may still have stream
        // state for it (it auto-creates a Type::Request entry on first read),
        // and we don't want to surface those phantom events to the caller.
        loop {
            match h3_conn.poll(conn) {
                Ok((stream_id, h3::Event::Headers { list, more_frames })) => {
                    if self.is_wt_classified(stream_id) {
                        continue;
                    }
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
                    if self.is_wt_classified(stream_id) {
                        continue;
                    }
                    events.push(WtEvent::H3Data { stream_id });
                }
                Ok((stream_id, h3::Event::Finished)) => {
                    if self.is_wt_classified(stream_id) {
                        continue;
                    }
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

        // Phase 3: Emit Finished for classified WT streams whose buffer has
        // drained empty after FIN was observed. We track FIN locally because
        // the bytes are consumed by drain_stream before h3 ever sees them.
        let fin_streams: Vec<u64> = self.stream_fin.iter()
            .filter_map(|(&sid, &fin)| {
                if !fin { return None; }
                if self.finished_reported.get(&sid).copied().unwrap_or(false) { return None; }
                let buf_empty = self.stream_data.get(&sid).map_or(true, |b| b.is_empty());
                if buf_empty { Some(sid) } else { None }
            })
            .collect();
        for sid in fin_streams {
            self.finished_reported.insert(sid, true);
            events.push(WtEvent::Finished { stream_id: sid });
        }

        events
    }

    fn is_wt_classified(&self, stream_id: u64) -> bool {
        matches!(
            self.streams.get(&stream_id),
            Some(StreamParseState::Ready { .. })
        )
    }

    /// Drain all readable bytes from a Ready WT stream into our internal
    /// `stream_data` buffer. Track FIN in `stream_fin`. Returns true if any
    /// bytes were read (so the caller can emit `WtEvent::Data`).
    fn drain_stream(&mut self, conn: &mut Connection, stream_id: u64) -> bool {
        // Don't drain if we haven't fully classified the stream — try_parse_prefix
        // owns reads until it produces a Ready state.
        if !matches!(
            self.streams.get(&stream_id),
            Some(StreamParseState::Ready { .. })
        ) {
            return false;
        }
        let mut tmp = [0u8; 4096];
        let mut got_data = false;
        loop {
            match conn.stream_recv(stream_id, &mut tmp) {
                Ok((n, fin)) => {
                    if n > 0 {
                        self.stream_data
                            .entry(stream_id)
                            .or_default()
                            .extend_from_slice(&tmp[..n]);
                        got_data = true;
                    }
                    if fin {
                        self.stream_fin.insert(stream_id, true);
                        break;
                    }
                    if n == 0 {
                        break;
                    }
                }
                Err(quiche::Error::Done) => break,
                Err(_) => {
                    break;
                }
            }
        }
        got_data
    }

    /// Read previously-drained bytes from the per-stream buffer. Returns the
    /// number of bytes copied into `out` (0 if buffer empty).
    /// Use `stream_fin_reached` to detect FIN once buffer is empty.
    pub fn stream_recv(&mut self, stream_id: u64, out: &mut [u8]) -> usize {
        let buf = match self.stream_data.get_mut(&stream_id) {
            Some(b) => b,
            None => return 0,
        };
        let n = std::cmp::min(buf.len(), out.len());
        if n == 0 {
            return 0;
        }
        out[..n].copy_from_slice(&buf[..n]);
        buf.drain(..n);
        n
    }

    /// Returns true if FIN has been observed on this stream and all buffered
    /// bytes have been consumed via `stream_recv`.
    pub fn stream_fin_reached(&self, stream_id: u64) -> bool {
        if !self.stream_fin.get(&stream_id).copied().unwrap_or(false) {
            return false;
        }
        self.stream_data
            .get(&stream_id)
            .map_or(true, |b| b.is_empty())
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
        self.stream_data.remove(&stream_id);
        self.stream_fin.remove(&stream_id);
        self.finished_reported.remove(&stream_id);
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
    ///
    /// Critical invariant: only consume the prefix bytes from the QUIC stream.
    /// Any post-prefix payload bytes must remain in conn so the caller can read
    /// them via `conn.stream_recv`. We use `parse_bufs` only to stash partial
    /// varint bytes when a prefix is split across QUIC packets.
    fn try_parse_prefix(
        &mut self,
        conn: &mut Connection,
        stream_id: u64,
    ) -> Option<WtEvent> {
        loop {
            // Read state by value so we don't hold a borrow on self.streams
            // across the read_varint_incremental call (which mutates parse_bufs).
            let state = match self.streams.get(&stream_id) {
                Some(StreamParseState::AwaitingType) => StreamParseState::AwaitingType,
                Some(StreamParseState::AwaitingSessionId { bidi }) =>
                    StreamParseState::AwaitingSessionId { bidi: *bidi },
                Some(StreamParseState::Ready { .. }) | None => return None,
            };

            match state {
                StreamParseState::AwaitingType => {
                    let val = self.read_varint_incremental(conn, stream_id)?;
                    let bidi = match val {
                        WT_STREAM_TYPE_BIDI => true,
                        WT_STREAM_TYPE_UNI => false,
                        _ => {
                            // Not a WT stream — drop tracking. The type-varint bytes
                            // are already consumed from conn; h3 cannot recover them.
                            self.streams.remove(&stream_id);
                            self.parse_bufs.remove(&stream_id);
                            return None;
                        }
                    };
                    self.streams.insert(stream_id, StreamParseState::AwaitingSessionId { bidi });
                    // Loop and try to parse the session ID (it may be in the same packet,
                    // or stream_recv will return Done and we'll bail until next poll).
                }
                StreamParseState::AwaitingSessionId { bidi } => {
                    let session_id = self.read_varint_incremental(conn, stream_id)?;
                    self.streams.insert(
                        stream_id,
                        StreamParseState::Ready { session_id, bidi },
                    );
                    // parse_bufs for this stream should be empty now; remove the entry
                    // so future `conn.stream_recv` calls go straight through.
                    self.parse_bufs.remove(&stream_id);
                    return Some(WtEvent::NewStream {
                        session_id,
                        stream_id,
                        bidi,
                    });
                }
                StreamParseState::Ready { .. } => return None,
            }
        }
    }

    /// Read exactly one QUIC varint from a stream, consuming only its bytes.
    /// Uses `parse_bufs[stream_id]` to stash partial bytes across calls when the
    /// varint is split across packets. Returns Some(value) when complete, None
    /// when more bytes are needed (and partial bytes are saved for next time).
    fn read_varint_incremental(
        &mut self,
        conn: &mut Connection,
        stream_id: u64,
    ) -> Option<u64> {
        // Pull any previously-buffered partial bytes out of parse_bufs.
        let mut accum = self.parse_bufs.remove(&stream_id).unwrap_or_default();

        // Step 1: ensure we have the leading byte to determine total varint length.
        if accum.is_empty() {
            let mut first = [0u8; 1];
            match conn.stream_recv(stream_id, &mut first) {
                Ok((1, _)) => {
                    accum.push(first[0]);
                }
                Ok(_) => {
                    return None;
                }
                Err(_) => {
                    return None;
                }
            }
        }

        let len = 1usize << ((accum[0] >> 6) as usize);

        // Step 2: read the remaining length bytes (one syscall per attempt is fine —
        // QUIC streams will deliver everything available in a single call up to N).
        while accum.len() < len {
            let need = len - accum.len();
            let mut tmp = vec![0u8; need];
            match conn.stream_recv(stream_id, &mut tmp) {
                Ok((n, _)) if n > 0 => {
                    accum.extend_from_slice(&tmp[..n]);
                }
                Ok(_) => {
                    self.parse_bufs.insert(stream_id, accum);
                    return None;
                }
                Err(_) => {
                    self.parse_bufs.insert(stream_id, accum);
                    return None;
                }
            }
        }

        // Step 3: decode.
        let mut val = (accum[0] & 0x3f) as u64;
        for i in 1..len {
            val = (val << 8) | (accum[i] as u64);
        }

        // Step 4: drain consumed bytes. accum should be exactly `len` bytes here,
        // but be defensive: if we somehow over-read, keep the leftover for the
        // next varint (shouldn't happen with the per-byte/exact-len reads above).
        accum.drain(..len);
        if !accum.is_empty() {
            self.parse_bufs.insert(stream_id, accum);
        }

        Some(val)
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
