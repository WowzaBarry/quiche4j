package io.quiche4j.http3;

import io.quiche4j.Native;

import java.nio.ByteBuffer;
import java.util.Map;

public final class Http3ConfigBuilder {

    private boolean enableExtendedConnect = false;
    private Map<Long, Long> additionalSettings = null;

    /**
     * Enable or disable Extended CONNECT protocol support (RFC 9220).
     * Required for WebTransport over HTTP/3.
     */
    public final Http3ConfigBuilder enableExtendedConnect(boolean enabled) {
        this.enableExtendedConnect = enabled;
        return this;
    }

    /**
     * Set additional HTTP/3 SETTINGS parameters.
     * Used for WebTransport settings like WT_MAX_SESSIONS, H3_DATAGRAM, etc.
     *
     * <p>Must not include settings already managed by quiche:
     * QPACK_MAX_TABLE_CAPACITY, MAX_FIELD_SECTION_SIZE, QPACK_BLOCKED_STREAMS,
     * ENABLE_CONNECT_PROTOCOL, H3_DATAGRAM.
     */
    public final Http3ConfigBuilder setAdditionalSettings(Map<Long, Long> settings) {
        this.additionalSettings = settings;
        return this;
    }

    /**
     * Creates a new {@link Http3Config} object with the configured settings.
     */
    public final Http3Config build() {
        final long ptr = Http3Native.quiche_h3_config_new();
        final Http3Config config = new Http3Config(ptr);
        Native.registerCleaner(config, () -> Http3Native.quiche_h3_config_free(ptr));

        if (enableExtendedConnect) {
            Http3Native.quiche_h3_config_enable_extended_connect(ptr, true);
        }

        if (additionalSettings != null && !additionalSettings.isEmpty()) {
            ByteBuffer keysBuf = ByteBuffer.allocate(additionalSettings.size() * 8);
            ByteBuffer valsBuf = ByteBuffer.allocate(additionalSettings.size() * 8);
            for (Map.Entry<Long, Long> entry : additionalSettings.entrySet()) {
                keysBuf.putLong(entry.getKey());
                valsBuf.putLong(entry.getValue());
            }
            Http3Native.quiche_h3_config_set_additional_settings(ptr, keysBuf.array(), valsBuf.array());
        }

        return config;
    }

}
