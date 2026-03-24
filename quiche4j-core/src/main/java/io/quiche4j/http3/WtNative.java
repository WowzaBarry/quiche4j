package io.quiche4j.http3;

import static io.quiche4j.Native.LIBRARY_NAME;

import io.quiche4j.NativeUtils;

/**
 * Native JNI declarations for WebTransport server operations.
 */
public class WtNative {

    static {
        try {
            System.loadLibrary(LIBRARY_NAME);
        } catch (java.lang.UnsatisfiedLinkError e) {
            NativeUtils.loadEmbeddedLibrary(LIBRARY_NAME);
        }
    }

    public final static native long wt_server_new();

    public final static native void wt_server_free(long wt_ptr);

    public final static native int wt_server_poll(long wt_ptr, long h3_conn_ptr, long conn_ptr, WtEventListener listener);

    public final static native long wt_server_open_uni_stream(long wt_ptr, long conn_ptr, long session_id);

    public final static native long wt_server_open_bidi_stream(long wt_ptr, long conn_ptr, long session_id);

    public final static native long wt_server_stream_send(long conn_ptr, long stream_id, byte[] buf, boolean fin);

    public final static native int wt_server_stream_recv(long conn_ptr, long stream_id, byte[] buf);

}
