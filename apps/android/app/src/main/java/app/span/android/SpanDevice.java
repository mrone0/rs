package app.span.android;

final class SpanDevice {
    String id;
    String name;
    String platform;
    String host;
    String publicKeyHex;
    boolean trusted;
    long lastSeenMillis;

    SpanDevice(String id, String name, String platform, String host, String publicKeyHex, boolean trusted, long lastSeenMillis) {
        this.id = id;
        this.name = name;
        this.platform = platform;
        this.host = host;
        this.publicKeyHex = publicKeyHex;
        this.trusted = trusted;
        this.lastSeenMillis = lastSeenMillis;
    }
}
