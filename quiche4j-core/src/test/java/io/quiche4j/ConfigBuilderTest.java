package io.quiche4j;

import java.net.URL;
import java.nio.file.Files;

import junit.framework.TestCase;

public class ConfigBuilderTest extends TestCase {

    private static String testCertPath() {
        final URL url = ConfigBuilderTest.class.getClassLoader().getResource("cert.crt");
        assertNotNull("cert.crt must be on the test classpath", url);
        return url.getFile();
    }

    public void testLoadVerifyLocationsFromFileBuildsSuccessfully() throws Exception {
        final Config config = new ConfigBuilder(Quiche.PROTOCOL_VERSION)
                .loadVerifyLocationsFromFile(testCertPath())
                .withVerifyPeer(true)
                .build();
        assertNotNull(config);
    }

    public void testLoadVerifyLocationsFromFileThrowsOnInvalidPath() throws Exception {
        final String bogus = Files.createTempFile("quiche4j-bogus", ".pem").toString();
        Files.delete(java.nio.file.Paths.get(bogus));

        try {
            new ConfigBuilder(Quiche.PROTOCOL_VERSION)
                    .loadVerifyLocationsFromFile(bogus)
                    .build();
            fail("Expected IllegalArgumentException for missing verify-location file");
        } catch (IllegalArgumentException expected) {
            assertTrue(expected.getMessage().contains(bogus));
        }
    }

    public void testLoadVerifyLocationsFromDirectoryBuildsSuccessfully() throws Exception {
        // Use a system directory that's guaranteed to exist and be openable; BoringSSL only
        // enumerates it lazily during verification, so we just check that the call
        // is wired up and the builder proceeds without throwing.
        final String dir = java.nio.file.Files.createTempDirectory("quiche4j-verify-dir").toString();
        final Config config = new ConfigBuilder(Quiche.PROTOCOL_VERSION)
                .loadVerifyLocationsFromDirectory(dir)
                .build();
        assertNotNull(config);
    }
}
