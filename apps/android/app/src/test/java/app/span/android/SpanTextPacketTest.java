package app.span.android;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;

import org.junit.Test;

public final class SpanTextPacketTest {
    @Test public void androidTextPacketRoundTrips() throws Exception {
        LocalIdentity sender = SpanCrypto.createIdentity("sender");
        LocalIdentity receiver = SpanCrypto.createIdentity("receiver");
        String text = "MFA 123456 / 中文 / https://span.local";

        SpanCrypto.Encrypted encrypted = SpanCrypto.encryptText(
                text, sender.privateKeyHex, receiver.publicKeyHex);
        String line = SpanProtocol.TEXT_MAGIC + "\t" + sender.id + "\t"
                + encrypted.nonceHex + "\t" + encrypted.ciphertextHex + "\n";

        SpanTextPacket packet = SpanTextPacket.parse(line);
        assertNotNull(packet);
        assertEquals(sender.id, packet.fromDeviceId);
        assertEquals(text, SpanCrypto.decryptText(packet, receiver.privateKeyHex, sender.publicKeyHex));
    }

    @Test public void malformedPacketsAreRejected() {
        assertEquals(null, SpanTextPacket.parse("bad\tpacket\n"));
        assertEquals(null, SpanTextPacket.parse(
                SpanProtocol.TEXT_MAGIC + "\tid\t00\t00\n"));
    }
}
