package app.span.android;

final class Hex {
    private Hex() {}

    static String encode(byte[] bytes) {
        char[] out = new char[bytes.length * 2];
        char[] alphabet = "0123456789abcdef".toCharArray();
        for (int i = 0; i < bytes.length; i++) {
            int v = bytes[i] & 0xff;
            out[i * 2] = alphabet[v >>> 4];
            out[i * 2 + 1] = alphabet[v & 0x0f];
        }
        return new String(out);
    }

    static byte[] decode(String value) {
        String s = value == null ? "" : value.trim();
        if ((s.length() & 1) != 0) return null;
        byte[] out = new byte[s.length() / 2];
        for (int i = 0; i < out.length; i++) {
            int hi = Character.digit(s.charAt(i * 2), 16);
            int lo = Character.digit(s.charAt(i * 2 + 1), 16);
            if (hi < 0 || lo < 0) return null;
            out[i] = (byte) ((hi << 4) | lo);
        }
        return out;
    }
}
