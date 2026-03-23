package io.quiche4j.http3;

import io.quiche4j.Connection;
import io.quiche4j.Native;

/**
 * WebTransport server that wraps an H3 connection and handles WT stream
 * type prefix parsing, session routing, and event dispatch natively in Rust.
 *
 * <p>Usage:
 * <pre>
 *   Http3Config h3Config = new Http3ConfigBuilder()
 *       .enableExtendedConnect(true)
 *       .setAdditionalSettings(Map.of(
 *           0xc671706aL, 1L,  // WT_MAX_SESSIONS
 *       ))
 *       .build();
 *   Http3Connection h3Conn = Http3Connection.withTransport(conn, h3Config);
 *   WtServer wt = new WtServer();
 *
 *   // In your event loop:
 *   int count = wt.poll(h3Conn, conn, listener);
 *   long streamId = wt.openUniStream(conn, sessionId);
 *   wt.streamSend(conn, streamId, data, false);
 * </pre>
 */
public final class WtServer {

    private final long ptr;

    public WtServer() {
        this.ptr = WtNative.wt_server_new();
        Native.registerCleaner(this, () -> WtNative.wt_server_free(this.ptr));
    }

    /**
     * Poll for WebTransport and H3 events, invoking callbacks on the listener.
     * Returns the number of events processed.
     */
    public int poll(Http3Connection h3Conn, Connection conn, WtEventListener listener) {
        return WtNative.wt_server_poll(ptr, h3Conn.getPointer(), conn.getPointer(), listener);
    }

    /**
     * Open a server-initiated unidirectional WT stream.
     * The 0x54 type prefix + session ID are written automatically.
     * Returns the QUIC stream ID, or -1 on error.
     */
    public long openUniStream(Connection conn, long sessionId) {
        return WtNative.wt_server_open_uni_stream(ptr, conn.getPointer(), sessionId);
    }

    /**
     * Open a server-initiated bidirectional WT stream.
     * The 0x41 type prefix + session ID are written automatically.
     * Returns the QUIC stream ID, or -1 on error.
     */
    public long openBidiStream(Connection conn, long sessionId) {
        return WtNative.wt_server_open_bidi_stream(ptr, conn.getPointer(), sessionId);
    }

    /**
     * Send data on a WT stream (raw payload, no prefix).
     * Returns bytes written, or negative error code.
     */
    public long streamSend(Connection conn, long streamId, byte[] buf, boolean fin) {
        return WtNative.wt_server_stream_send(conn.getPointer(), streamId, buf, fin);
    }

    /**
     * Receive data from a WT stream into the provided buffer.
     * Returns bytes read, or negative error code.
     */
    public int streamRecv(Connection conn, long streamId, byte[] buf) {
        return WtNative.wt_server_stream_recv(conn.getPointer(), streamId, buf);
    }

}
