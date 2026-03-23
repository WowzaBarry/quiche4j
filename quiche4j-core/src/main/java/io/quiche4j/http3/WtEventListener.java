package io.quiche4j.http3;

import java.util.List;

/**
 * Callback interface for WebTransport server events.
 *
 * <p>Extends {@link Http3EventListener} so that standard H3 events
 * (headers, data, finished) are also delivered for non-WT streams.
 */
public interface WtEventListener extends Http3EventListener {

    /**
     * A new WebTransport session was established via Extended CONNECT.
     * The 200 OK response has already been sent.
     *
     * @param sessionId the WT session ID (= the H3 request stream ID)
     * @param path the :path from the CONNECT request (e.g. "/moq")
     */
    void onNewSession(long sessionId, String path);

    /**
     * A new WebTransport data stream was opened by the peer.
     * The stream type prefix and session ID have been parsed.
     *
     * @param sessionId the WT session this stream belongs to
     * @param streamId the QUIC stream ID
     * @param bidi true if bidirectional, false if unidirectional
     */
    void onNewStream(long sessionId, long streamId, boolean bidi);

}
