package app.span.android;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.charset.StandardCharsets;

final class SpanTransport {
    private static final int MAX_ATTEMPTS = 3;
    private static final int CONNECT_TIMEOUT_MILLIS = 1800;
    private static final int ACK_TIMEOUT_MILLIS = 1800;
    private static final String ACK = "SPAN_OK";

    void sendText(String text, LocalIdentity identity, SpanDevice device) throws Exception {
        if (text == null || text.isEmpty()) return;
        if (text.getBytes(StandardCharsets.UTF_8).length > SpanProtocol.MAX_TEXT_BYTES) {
            throw new IllegalArgumentException("text too large");
        }
        if (device.host == null || device.host.trim().isEmpty()) throw new IllegalArgumentException("missing host");
        if (device.publicKeyHex == null || device.publicKeyHex.trim().isEmpty()) throw new IllegalArgumentException("missing key");
        SpanCrypto.Encrypted encrypted = SpanCrypto.encryptText(text, identity.privateKeyHex, device.publicKeyHex);
        String line = SpanProtocol.TEXT_MAGIC + "\t" + identity.id + "\t" + encrypted.nonceHex + "\t" + encrypted.ciphertextHex + "\n";
        byte[] data = line.getBytes(StandardCharsets.UTF_8);
        Exception lastFailure = null;
        for (int attempt = 1; attempt <= MAX_ATTEMPTS; attempt++) {
            try (Socket socket = new Socket()) {
                socket.connect(
                        new InetSocketAddress(device.host, SpanProtocol.TEXT_PORT),
                        CONNECT_TIMEOUT_MILLIS);
                socket.setTcpNoDelay(true);
                socket.setSoTimeout(ACK_TIMEOUT_MILLIS);
                OutputStream out = socket.getOutputStream();
                out.write(data);
                out.flush();

                BufferedReader response = new BufferedReader(new InputStreamReader(
                        socket.getInputStream(), StandardCharsets.UTF_8));
                if (!ACK.equals(response.readLine())) {
                    throw new java.io.IOException("receiver did not acknowledge clipboard text");
                }
                return;
            } catch (Exception error) {
                lastFailure = error;
                if (attempt < MAX_ATTEMPTS) Thread.sleep(180L * attempt);
            }
        }
        throw lastFailure == null
                ? new java.io.IOException("clipboard send failed")
                : lastFailure;
    }
}
