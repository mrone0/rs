package app.span.android;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;

import java.util.Arrays;
import java.util.Random;
import org.junit.Test;

public final class X25519Test {
    @Test public void matchesRfc7748TestVectors() {
        assertEquals(
                "c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552",
                Hex.encode(X25519.shared(
                        Hex.decode("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4"),
                        Hex.decode("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c"))));

        assertEquals(
                "95cbde9476e8907d7aade45cb4b873f88b595a68799fa152e6f8f7647aac7957",
                Hex.encode(X25519.shared(
                        Hex.decode("4b66e9d4d1b4673c5ad22691957d6af5c11b6421e0ea01d42ca4169e7918ba0d"),
                        Hex.decode("e5210f12786811d3f4b7959d0538ae2c31dbe7106fc03c3efc4cd549c715a493"))));
    }

    @Test public void sharedSecretIsSymmetric() {
        Random random = new Random(1L);
        for (int i = 0; i < 100; i++) {
            byte[] alicePrivate = new byte[32];
            byte[] bobPrivate = new byte[32];
            random.nextBytes(alicePrivate);
            random.nextBytes(bobPrivate);

            byte[] alicePublic = X25519.publicFromPrivate(alicePrivate);
            byte[] bobPublic = X25519.publicFromPrivate(bobPrivate);

            assertArrayEquals(
                    "shared secret mismatch at iteration " + i,
                    X25519.shared(alicePrivate, bobPublic),
                    X25519.shared(bobPrivate, alicePublic));
            assertEquals(32, alicePublic.length);
            assertEquals(32, bobPublic.length);
        }
    }

    @Test public void randomPrivateKeyIsClamped() {
        byte[] privateKey = X25519.randomPrivateKey();
        assertEquals(32, privateKey.length);
        assertEquals(0, privateKey[0] & 7);
        assertEquals(0, privateKey[31] & 0x80);
        assertEquals(0x40, privateKey[31] & 0x40);
        Arrays.fill(privateKey, (byte) 0);
    }
}
