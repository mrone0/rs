package app.span.android;

import java.math.BigInteger;
import java.security.SecureRandom;
import java.util.Arrays;

final class X25519 {
    private static final BigInteger P = BigInteger.ONE.shiftLeft(255).subtract(BigInteger.valueOf(19));
    private static final BigInteger A24 = BigInteger.valueOf(121665);
    private static final BigInteger BASE_U = BigInteger.valueOf(9);

    private X25519() {}

    static byte[] randomPrivateKey() {
        byte[] k = new byte[32];
        new SecureRandom().nextBytes(k);
        clamp(k);
        return k;
    }

    static byte[] publicFromPrivate(byte[] privateKey) {
        return scalarMult(privateKey, littleEndian(BASE_U));
    }

    static byte[] shared(byte[] privateKey, byte[] peerPublicKey) {
        return scalarMult(privateKey, peerPublicKey);
    }

    private static byte[] scalarMult(byte[] scalar, byte[] uBytes) {
        if (scalar == null || scalar.length != 32) throw new IllegalArgumentException("bad scalar");
        if (uBytes == null || uBytes.length != 32) throw new IllegalArgumentException("bad u-coordinate");
        byte[] k = Arrays.copyOf(scalar, 32);
        clamp(k);
        byte[] u = Arrays.copyOf(uBytes, 32);
        // RFC 7748 section 5: X25519 implementations mask the most significant
        // bit of the final byte when decoding a u-coordinate.
        u[31] &= 0x7f;
        BigInteger x1 = fromLittleEndian(u).mod(P);
        BigInteger x2 = BigInteger.ONE;
        BigInteger z2 = BigInteger.ZERO;
        BigInteger x3 = x1;
        BigInteger z3 = BigInteger.ONE;
        int swap = 0;

        for (int t = 254; t >= 0; t--) {
            int kt = ((k[t >>> 3] & 0xff) >>> (t & 7)) & 1;
            swap ^= kt;
            if (swap != 0) {
                BigInteger tmp = x2; x2 = x3; x3 = tmp;
                tmp = z2; z2 = z3; z3 = tmp;
            }
            swap = kt;

            BigInteger a = x2.add(z2).mod(P);
            BigInteger aa = a.multiply(a).mod(P);
            BigInteger b = x2.subtract(z2).mod(P);
            BigInteger bb = b.multiply(b).mod(P);
            BigInteger e = aa.subtract(bb).mod(P);
            BigInteger c = x3.add(z3).mod(P);
            BigInteger d = x3.subtract(z3).mod(P);
            BigInteger da = d.multiply(a).mod(P);
            BigInteger cb = c.multiply(b).mod(P);
            BigInteger daPlusCb = da.add(cb).mod(P);
            BigInteger daMinusCb = da.subtract(cb).mod(P);
            x3 = daPlusCb.multiply(daPlusCb).mod(P);
            z3 = x1.multiply(daMinusCb.multiply(daMinusCb).mod(P)).mod(P);
            x2 = aa.multiply(bb).mod(P);
            z2 = e.multiply(aa.add(A24.multiply(e)).mod(P)).mod(P);
        }
        if (swap != 0) {
            BigInteger tmp = x2; x2 = x3; x3 = tmp;
            tmp = z2; z2 = z3; z3 = tmp;
        }
        BigInteger result = x2.multiply(z2.modInverse(P)).mod(P);
        return littleEndian(result);
    }

    private static void clamp(byte[] k) {
        k[0] &= 248;
        k[31] &= 127;
        k[31] |= 64;
    }

    private static BigInteger fromLittleEndian(byte[] in) {
        byte[] reversed = new byte[in.length + 1];
        // BigInteger expects big-endian two's-complement. Keep byte 0 as a
        // positive sign byte, then reverse the little-endian input after it.
        for (int i = 0; i < in.length; i++) reversed[in.length - i] = in[i];
        return new BigInteger(reversed);
    }

    private static byte[] littleEndian(BigInteger value) {
        byte[] big = value.mod(P).toByteArray();
        byte[] out = new byte[32];
        for (int i = 0; i < big.length; i++) {
            int src = big.length - 1 - i;
            if (src >= 0 && i < 32) out[i] = big[src];
        }
        return out;
    }
}
