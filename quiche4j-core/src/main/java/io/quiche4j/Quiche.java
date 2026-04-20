package io.quiche4j;

import java.net.InetSocketAddress;
import java.security.InvalidKeyException;
import java.security.NoSuchAlgorithmException;
import java.util.Arrays;
import java.util.Random;
import javax.crypto.KeyGenerator;
import javax.crypto.Mac;
import javax.crypto.SecretKey;
import javax.crypto.spec.SecretKeySpec;

public final class Quiche {

    /**
     * The maximum length of a connection ID.
     */
    public static final int MAX_CONN_ID_LEN = 20;

    /**
     * The alogithm name for {@code KeyGenerator} to generate secret
     * spec to be used as a connection ID seed.
     */
    public static final String CONN_ID_SEED_ALGO = "HMACSHA256";

    /**
     * A listing of QUIC error codes.
     */
    public static final class ErrorCode {
        /**
         * No errors. All good.
         */
        public static final short SUCCESS = 0;

        /**
         * There is no more work to do.
         */
        public static final short DONE = -1;

        /**
         * The provided buffer is too short.
         */
        public static final short BUFFER_TOO_SHORT = -2;

        /**
         * The provided packet cannot be parsed because its version is unknown.
         */
        public static final short UNKNOWN_VERSION = -3;

        /**
         * The provided packet cannot be parsed because it contains an invalid
         * frame.
         */
        public static final short INVALID_FRAME = -4;

        /**
         * The provided packet cannot be parsed.
         */
        public static final short INVALID_PACKET = -5;

        /**
         * The operation cannot be completed because the connection is in an
         * invalid state.
         */
        public static final short INVALID_STATE = -6;

        /**
         * The operation cannot be completed because the stream is in an
         * invalid state.
         */
        public static final short INVALID_STREAM_STATE = -7;

        /**
         * The peer's transport params cannot be parsed.
         */
        public static final short INVALID_TRANSPORT_PARAM = -8;

        /**
         * A cryptographic operation failed.
         */
        public static final short CRYPTO_FAIL = -9;

        /**
         * The TLS handshake failed.
         */
        public static final short TLS_FAIL = -10;

        /**
         * The peer violated the local flow control limits.
         */
        public static final short FLOW_CONTROL = -11;

        /**
         * The peer violated the local stream limits.
         */
        public static final short STREAM_LIMIT = -12;

        /**
         * The received data exceeds the stream's final size.
         */
        public static final short FINAL_SIZE = -13;

        /**
         * Error in congestion control.
         */
        public static final short CONGESTION_CONTROL = -14;

        /**
         * The specified stream was stopped by the peer.
         */
        public static final short STREAM_STOPPED = -15;

        /**
         * The specified stream was reset by the peer.
         */
        public static final short STREAM_RESET = -16;

        /**
         * Too many identifiers were provided.
         */
        public static final short ID_LIMIT = -17;

        /**
         * Not enough available identifiers.
         */
        public static final short OUT_OF_IDENTIFIERS = -18;

        /**
         * Error in key update.
         */
        public static final short KEY_UPDATE = -19;

        /**
         * The peer sent more data in CRYPTO frames than we can buffer.
         */
        public static final short CRYPTO_BUFFER_EXCEEDED = -20;

        /**
         * The peer sent an ACK frame with an invalid range.
         */
        public static final short INVALID_ACK_RANGE = -21;

        /**
         * The peer is acknowledging packets that haven't been sent yet.
         */
        public static final short OPTIMISTIC_ACK_DETECTED = -22;

        /**
         * The peer's first flight used an invalid destination connection ID.
         */
        public static final short INVALID_DCID_INITIALIZATION = -23;

        /**
         * Lower bound on native error codes. Values in {@code [MIN_ERROR_CODE, 0]} are
         * interpreted as error sentinels rather than native pointers.
         *
         * <p>Native constructors return the error code as a {@code jlong} from the
         * same slot that, on success, carries a {@code Box::into_raw} heap pointer.
         * Treating every negative {@code jlong} as an error is unsafe on platforms
         * where pointers can have bit 63 set (e.g. Android arm64 with MTE tagging
         * via Scudo). Since {@code error_to_c} currently only emits values down to
         * {@code -23}, pointers will never fall into {@code [-100, 0]}; we use this
         * constant rather than the exact tightest bound to leave room for new quiche
         * error variants without requiring a simultaneous Java-side update.
         */
        public static final long MIN_ERROR_CODE = -100L;

        /**
         * Returns {@code true} if the given native return value should be treated as
         * an error rather than a valid pointer. See {@link #MIN_ERROR_CODE} for why
         * a bare {@code ptr <= 0} check is unsafe on Android arm64.
         */
        public static boolean isErrorCode(long code) {
            return code == 0L || (code < 0L && code > MIN_ERROR_CODE);
        }

        /**
         * Returns the symbolic name for a native error code, or {@code "UNKNOWN"}
         * if the code is not recognised.
         *
         * <p>Useful for error messages, logging, and exception text:
         * <pre>
         *     Quiche.ErrorCode.name(-10) // returns "TLS_FAIL"
         * </pre>
         */
        public static String name(long code) {
            if (code == SUCCESS) return "SUCCESS";
            if (code == DONE) return "DONE";
            if (code == BUFFER_TOO_SHORT) return "BUFFER_TOO_SHORT";
            if (code == UNKNOWN_VERSION) return "UNKNOWN_VERSION";
            if (code == INVALID_FRAME) return "INVALID_FRAME";
            if (code == INVALID_PACKET) return "INVALID_PACKET";
            if (code == INVALID_STATE) return "INVALID_STATE";
            if (code == INVALID_STREAM_STATE) return "INVALID_STREAM_STATE";
            if (code == INVALID_TRANSPORT_PARAM) return "INVALID_TRANSPORT_PARAM";
            if (code == CRYPTO_FAIL) return "CRYPTO_FAIL";
            if (code == TLS_FAIL) return "TLS_FAIL";
            if (code == FLOW_CONTROL) return "FLOW_CONTROL";
            if (code == STREAM_LIMIT) return "STREAM_LIMIT";
            if (code == FINAL_SIZE) return "FINAL_SIZE";
            if (code == CONGESTION_CONTROL) return "CONGESTION_CONTROL";
            if (code == STREAM_STOPPED) return "STREAM_STOPPED";
            if (code == STREAM_RESET) return "STREAM_RESET";
            if (code == ID_LIMIT) return "ID_LIMIT";
            if (code == OUT_OF_IDENTIFIERS) return "OUT_OF_IDENTIFIERS";
            if (code == KEY_UPDATE) return "KEY_UPDATE";
            if (code == CRYPTO_BUFFER_EXCEEDED) return "CRYPTO_BUFFER_EXCEEDED";
            if (code == INVALID_ACK_RANGE) return "INVALID_ACK_RANGE";
            if (code == OPTIMISTIC_ACK_DETECTED) return "OPTIMISTIC_ACK_DETECTED";
            if (code == INVALID_DCID_INITIALIZATION) return "INVALID_DCID_INITIALIZATION";
            return "UNKNOWN";
        }
    }

    /**
     * QUIC v1 (RFC 9000).
     */
    public static final int PROTOCOL_VERSION_V1 = 0x0000_0001;

    /**
     * The current QUIC wire version.
     */
    public static final int PROTOCOL_VERSION = PROTOCOL_VERSION_V1;

    /**
     * The stream's side to shutdown.
     * 
     * <p>This should be used when calling {@link Connection#streamShutdown}.
     */
    public enum Shutdown {
        /**
         * Stop receiving stream data.
         */
        READ(0),
        
        /**
         * Stop sending stream data.
         */
        WRITE(1);

        private final int value;

        Shutdown(int value) {
            this.value = value;
        }

        public final int value() {
            return this.value;
        }
    }

    /**
     * Generate random connection ID.
     */
    public static final byte[] newConnectionId() {
        return newConnectionId(new Random());
    }
 
    /**
     * Generate random connection ID using given generator of
     * pseudorandom numbers.
     */
    public static final byte[] newConnectionId(Random rnd) {
        final byte[] connId = new byte[MAX_CONN_ID_LEN];
        rnd.nextBytes(connId);
        return connId;
    }

    /**
     * Generate new random connection ID seed.
     */
    public static final byte[] newConnectionIdSeed() {
        try {
            final SecretKey key = KeyGenerator.getInstance(CONN_ID_SEED_ALGO).generateKey();
            return key.getEncoded();
        } catch (NoSuchAlgorithmException e) {
            throw new RuntimeException("Failed to generate secret key");
        }
    }

    /**
     * Sign connection ID using given connection ID seed.
     */
    public static final byte[] signConnectionId(byte[] seed, byte[] data) {
        final SecretKeySpec keySpec = new SecretKeySpec(seed, CONN_ID_SEED_ALGO);
        Mac mac;
        try {
            mac = Mac.getInstance(CONN_ID_SEED_ALGO);
            mac.init(keySpec);
        } catch (NoSuchAlgorithmException | InvalidKeyException e) {
            throw new RuntimeException("Failed to sign connection ID");
        }
        final byte[] signed = mac.doFinal(data);
        return Arrays.copyOfRange(signed, 0, MAX_CONN_ID_LEN);
    }

    /**
     * Writes a version negotiation packet.
     * 
     * <p>The {@code sourceConnId} and {@code destinationConnId} parameters are the
     * source connection ID and the destination connection ID extracted from the
     * received client's {@link PacketType.Initial} packet that advertises an unsupported version.
     */
    public static final int negotiateVersion(byte[] sourceConnId, byte[] destinationConnId, byte[] buf) {
        return Native.quiche_negotiate_version(sourceConnId, destinationConnId, buf);
    }

    /**
     * Returns {@code true} if the given protocol version is supported.
     */
    public static final boolean versionIsSupported(int version) {
        return Native.quiche_version_is_supported(version);
    }

    /**
     * Writes a stateless retry packet.
     *
     * <p>The {@code sourceConnId} and {@code destinationConnId} parameters are the
     * source connection ID and the destination connection ID extracted from the
     * received client's {@link PacketType.Initial} packet. The server's new source connection ID and
     * {@code token} is the address validation token the client needs to echo back.
     *
     * <p>The application is responsible for generating the address validation token to
     * be sent to the client, and verifying tokens sent back by the client. The
     * generated token should include the {@code destinationConnId} parameter, such
     * that it can be later extracted from the token and passed to the
     * {@link Quiche#accept} function as its {@code originalDestinationConnId}
     * parameter.
     */
    public static final int retry(byte[] sourceConnId, byte[] destinationConnId, byte[] newSourceConnId, byte[] token,
            int version, byte[] buf) {
        return Native.quiche_retry(sourceConnId, destinationConnId, newSourceConnId, token, version, buf);
    }

    /**
     * Creates a new server-side connection.
     *
     * <p>
     * The {@code sourceConnId} parameter represents the server's source connection
     * ID, while the optional {@code originalDestinationConnId} parameter represents
     * the original destination ID the client sent before a stateless retry (this is
     * only required when using the {@link Quiche#retry} function).
     * 
     * @throws ConnectionFailureException
     */
    public static final Connection accept(byte[] sourceConnId, byte[] originalDestinationConnId, Config config,
            InetSocketAddress localAddr, InetSocketAddress peerAddr) throws ConnectionFailureException {
        final long ptr = Native.quiche_accept(sourceConnId, originalDestinationConnId, config.getPointer(),
            localAddr.getAddress().getAddress(), localAddr.getPort(),
            peerAddr.getAddress().getAddress(), peerAddr.getPort());
        if (ErrorCode.isErrorCode(ptr)) {
            throw new ConnectionFailureException(ptr);
        }
        return Connection.newInstance(ptr);
    }

    /**
     * Creates a new client-side connection.
     *
     * The {@code sourceConnId} parameter is used as the connection's source
     * connection ID, while the optional {@code serverName} parameter is used to
     * verify the peer's certificate.
     *
     * @throws ConnectionFailureException
     */
    public static final Connection connect(String serverName, byte[] connId, Config config,
            InetSocketAddress localAddr, InetSocketAddress peerAddr) throws ConnectionFailureException {
        final long ptr = Native.quiche_connect(serverName, connId, config.getPointer(),
            localAddr.getAddress().getAddress(), localAddr.getPort(),
            peerAddr.getAddress().getAddress(), peerAddr.getPort());
        if (ErrorCode.isErrorCode(ptr)) {
            throw new ConnectionFailureException(ptr);
        }
        return Connection.newInstance(ptr);
    }

}