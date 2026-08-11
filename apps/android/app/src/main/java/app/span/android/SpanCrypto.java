package app.span.android;

import java.nio.charset.StandardCharsets;
import java.security.SecureRandom;
import java.util.Arrays;
import javax.crypto.Cipher;
import javax.crypto.Mac;
import javax.crypto.spec.IvParameterSpec;
import javax.crypto.spec.SecretKeySpec;

final class SpanCrypto {
    private SpanCrypto() {}

    static LocalIdentity createIdentity(String deviceName) throws Exception {
        byte[] privateKey = X25519.randomPrivateKey();
        byte[] publicKey = X25519.publicFromPrivate(privateKey);
        String safeName = sanitize(deviceName == null || deviceName.trim().isEmpty() ? "Android" : deviceName.trim());
        String id = safeName + "-" + System.currentTimeMillis();
        return new LocalIdentity(id, safeName, Hex.encode(privateKey), Hex.encode(publicKey));
    }

    static Encrypted encryptText(String text, String localPrivateKeyHex, String peerPublicKeyHex) throws Exception {
        byte[] privateKey = Hex.decode(localPrivateKeyHex);
        byte[] peerPublicKey = Hex.decode(peerPublicKeyHex);
        if (privateKey == null || privateKey.length != 32) throw new IllegalArgumentException("bad private key");
        if (peerPublicKey == null || peerPublicKey.length != 32) throw new IllegalArgumentException("bad peer public key");
        byte[] shared = X25519.shared(privateKey, peerPublicKey);
        byte[] key = hkdfSha256(shared, SpanProtocol.TEXT_KEY_INFO, 32);
        byte[] nonce = new byte[12];
        new SecureRandom().nextBytes(nonce);

        Cipher cipher = Cipher.getInstance("ChaCha20-Poly1305");
        cipher.init(Cipher.ENCRYPT_MODE, new SecretKeySpec(key, "ChaCha20"), new IvParameterSpec(nonce));
        byte[] ciphertextAndTag = cipher.doFinal(text.getBytes(StandardCharsets.UTF_8));
        return new Encrypted(Hex.encode(nonce), Hex.encode(ciphertextAndTag));
    }

    private static byte[] hkdfSha256(byte[] ikm, byte[] info, int len) throws Exception {
        byte[] salt = new byte[32];
        Mac mac = Mac.getInstance("HmacSHA256");
        mac.init(new SecretKeySpec(salt, "HmacSHA256"));
        byte[] prk = mac.doFinal(ikm);

        byte[] out = new byte[len];
        byte[] previous = new byte[0];
        int offset = 0;
        int counter = 1;
        while (offset < len) {
            mac.init(new SecretKeySpec(prk, "HmacSHA256"));
            mac.update(previous);
            mac.update(info);
            mac.update((byte) counter);
            previous = mac.doFinal();
            int copy = Math.min(previous.length, len - offset);
            System.arraycopy(previous, 0, out, offset, copy);
            offset += copy;
            counter++;
        }
        Arrays.fill(prk, (byte) 0);
        return out;
    }

    private static String sanitize(String value) {
        return value.replace('\t', ' ').replace('\n', ' ').replace('\r', ' ');
    }

    static final class Encrypted {
        final String nonceHex;
        final String ciphertextHex;
        Encrypted(String nonceHex, String ciphertextHex) {
            this.nonceHex = nonceHex;
            this.ciphertextHex = ciphertextHex;
        }
    }
}
