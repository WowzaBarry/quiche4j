package io.quiche4j;

import junit.framework.TestCase;

public class ConnectionFailureExceptionTest extends TestCase {

    public void testMessageIncludesSymbolicName() {
        final ConnectionFailureException e =
                new ConnectionFailureException(Quiche.ErrorCode.TLS_FAIL);
        assertEquals(Quiche.ErrorCode.TLS_FAIL, e.errorCode());
        assertTrue(
            "Expected message to contain symbolic name, got: " + e.getMessage(),
            e.getMessage().contains("TLS_FAIL"));
        assertTrue(
            "Expected message to contain numeric code, got: " + e.getMessage(),
            e.getMessage().contains(String.valueOf(Quiche.ErrorCode.TLS_FAIL)));
    }

    public void testMessageForUnknownCodeStillReadable() {
        final ConnectionFailureException e = new ConnectionFailureException(-999);
        assertTrue(e.getMessage().contains("UNKNOWN"));
        assertTrue(e.getMessage().contains("-999"));
    }
}
