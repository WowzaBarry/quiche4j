package io.quiche4j;

import java.net.InetAddress;
import java.net.InetSocketAddress;

import junit.framework.TestCase;

public class ConnectionTest extends TestCase {

    private static Connection newClientConnection() throws Exception {
        final Config config = new ConfigBuilder(Quiche.PROTOCOL_VERSION)
                .withApplicationProtos(new byte[] { 2, 'h', '3' })
                .withVerifyPeer(false)
                .build();
        final InetSocketAddress local =
                new InetSocketAddress(InetAddress.getByName("127.0.0.1"), 0);
        final InetSocketAddress peer =
                new InetSocketAddress(InetAddress.getByName("127.0.0.1"), 4433);
        return Quiche.connect("example.test", Quiche.newConnectionId(), config, local, peer);
    }

    public void testApplicationProtoIsEmptyBeforeHandshake() throws Exception {
        final Connection conn = newClientConnection();
        final byte[] proto = conn.applicationProto();
        assertNotNull(proto);
        assertEquals(0, proto.length);
    }

    public void testPeerCertificateChainIsNullBeforeHandshake() throws Exception {
        final Connection conn = newClientConnection();
        assertNull(conn.peerCertificateChain());
    }
}
