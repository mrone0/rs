package app.span.android;

import android.os.Handler;
import android.os.Looper;
import java.net.DatagramPacket;
import java.net.DatagramSocket;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.InterfaceAddress;
import java.net.NetworkInterface;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

final class SpanDiscovery {
    interface Listener { void onDevice(SpanDevice device); }

    private final LocalIdentity identity;
    private final Listener listener;
    private final ExecutorService executor = Executors.newSingleThreadExecutor();
    private volatile boolean running;
    private DatagramSocket socket;

    SpanDiscovery(LocalIdentity identity, Listener listener) {
        this.identity = identity;
        this.listener = listener;
    }

    void start() {
        if (running) return;
        running = true;
        executor.execute(this::runLoop);
    }

    void stop() {
        running = false;
        if (socket != null) socket.close();
    }

    void destroy() {
        stop();
        executor.shutdownNow();
    }

    void announceOnce() {
        new Thread(this::sendAnnouncement, "span-announce").start();
    }

    private void runLoop() {
        long nextAnnounce = 0;
        try {
            socket = new DatagramSocket(null);
            socket.setReuseAddress(true);
            socket.setBroadcast(true);
            socket.bind(new InetSocketAddress(InetAddress.getByName("0.0.0.0"), SpanProtocol.DISCOVERY_PORT));
            socket.setSoTimeout(500);
            byte[] buffer = new byte[2048];
            while (running) {
                long now = System.currentTimeMillis();
                if (now >= nextAnnounce) {
                    sendAnnouncement();
                    nextAnnounce = now + 15000;
                }
                try {
                    DatagramPacket packet = new DatagramPacket(buffer, buffer.length);
                    socket.receive(packet);
                    String value = new String(packet.getData(), 0, packet.getLength(), StandardCharsets.UTF_8);
                    SpanDevice device = parsePacket(value, packet.getAddress().getHostAddress());
                    if (device != null && !identity.id.equals(device.id)) {
                        new Handler(Looper.getMainLooper()).post(() -> listener.onDevice(device));
                    }
                } catch (java.net.SocketTimeoutException ignored) {
                } catch (Exception ignored) {
                }
            }
        } catch (Exception ignored) {
        } finally {
            if (socket != null) socket.close();
        }
    }

    private void sendAnnouncement() {
        try (DatagramSocket out = new DatagramSocket()) {
            out.setBroadcast(true);
            String payload = SpanProtocol.DISCOVERY_MAGIC + "\t" + identity.id + "\t" + sanitize(identity.name) + "\tandroid\t" + identity.publicKeyHex;
            byte[] data = payload.getBytes(StandardCharsets.UTF_8);
            boolean sent = false;
            java.util.Enumeration<NetworkInterface> interfaces = NetworkInterface.getNetworkInterfaces();
            while (interfaces != null && interfaces.hasMoreElements()) {
                NetworkInterface networkInterface = interfaces.nextElement();
                if (!networkInterface.isUp() || networkInterface.isLoopback()) continue;
                for (InterfaceAddress address : networkInterface.getInterfaceAddresses()) {
                    InetAddress broadcast = address.getBroadcast();
                    if (broadcast == null) continue;
                    DatagramPacket packet = new DatagramPacket(data, data.length, broadcast, SpanProtocol.DISCOVERY_PORT);
                    out.send(packet);
                    sent = true;
                }
            }
            if (!sent) {
                DatagramPacket packet = new DatagramPacket(data, data.length, InetAddress.getByName("255.255.255.255"), SpanProtocol.DISCOVERY_PORT);
                out.send(packet);
            }
        } catch (Exception ignored) {
        }
    }

    private SpanDevice parsePacket(String value, String host) {
        String[] parts = value.trim().split("\t", -1);
        if (parts.length < 5 || !SpanProtocol.DISCOVERY_MAGIC.equals(parts[0])) return null;
        byte[] key = Hex.decode(parts[4]);
        if (key == null || key.length != 32) return null;
        return new SpanDevice(parts[1], parts[2], parts[3], host, parts[4], false, System.currentTimeMillis());
    }

    private String sanitize(String s) {
        return s.replace('\t', ' ').replace('\n', ' ').replace('\r', ' ');
    }
}
