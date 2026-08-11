package app.span.android;

final class LocalIdentity {
    final String id;
    final String name;
    final String privateKeyHex;
    final String publicKeyHex;

    LocalIdentity(String id, String name, String privateKeyHex, String publicKeyHex) {
        this.id = id;
        this.name = name;
        this.privateKeyHex = privateKeyHex;
        this.publicKeyHex = publicKeyHex;
    }
}
