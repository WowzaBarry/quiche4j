package io.quiche4j;

import junit.framework.TestCase;

public class ErrorCodeTest extends TestCase {

    public void testNameCoversAllKnownCodes() {
        assertEquals("SUCCESS", Quiche.ErrorCode.name(Quiche.ErrorCode.SUCCESS));
        assertEquals("DONE", Quiche.ErrorCode.name(Quiche.ErrorCode.DONE));
        assertEquals("TLS_FAIL", Quiche.ErrorCode.name(Quiche.ErrorCode.TLS_FAIL));
        assertEquals("CRYPTO_BUFFER_EXCEEDED",
                Quiche.ErrorCode.name(Quiche.ErrorCode.CRYPTO_BUFFER_EXCEEDED));
        assertEquals("INVALID_ACK_RANGE",
                Quiche.ErrorCode.name(Quiche.ErrorCode.INVALID_ACK_RANGE));
        assertEquals("INVALID_DCID_INITIALIZATION",
                Quiche.ErrorCode.name(Quiche.ErrorCode.INVALID_DCID_INITIALIZATION));
    }

    public void testNameReturnsUnknownForUnmappedCode() {
        assertEquals("UNKNOWN", Quiche.ErrorCode.name(-999));
        assertEquals("UNKNOWN", Quiche.ErrorCode.name(1234));
    }

    public void testIsErrorCodeForZeroAndKnownSentinels() {
        assertTrue(Quiche.ErrorCode.isErrorCode(0L));
        assertTrue(Quiche.ErrorCode.isErrorCode(Quiche.ErrorCode.DONE));
        assertTrue(Quiche.ErrorCode.isErrorCode(Quiche.ErrorCode.TLS_FAIL));
        assertTrue(Quiche.ErrorCode.isErrorCode(Quiche.ErrorCode.INVALID_DCID_INITIALIZATION));
    }

    public void testIsErrorCodeIgnoresTaggedPointers() {
        // Simulate an Android arm64 MTE-tagged pointer: bit 63 set, but the low
        // 48 bits are a plausible heap address. The bare `ptr <= 0` check would
        // misclassify this as an error.
        final long taggedHeapPointer = 0x80000000_12345000L;
        assertFalse(Quiche.ErrorCode.isErrorCode(taggedHeapPointer));

        // Conventional x86_64 user-space pointer: positive.
        assertFalse(Quiche.ErrorCode.isErrorCode(0x7f_12345000L));
    }

    public void testIsErrorCodeFencePosts() {
        // MIN_ERROR_CODE itself is outside the error range (strictly greater than).
        assertFalse(Quiche.ErrorCode.isErrorCode(Quiche.ErrorCode.MIN_ERROR_CODE));
        // Just inside the error range.
        assertTrue(Quiche.ErrorCode.isErrorCode(Quiche.ErrorCode.MIN_ERROR_CODE + 1));
    }
}
