package app.span.android;

import android.content.Context;
import android.net.wifi.WifiManager;
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

    private final Context context;
    private final LocalIdentity identity;
    private final Listener listener;
    private final ExecutorService executor = Executors.newSingleThreadExecutor();
    private final Handler mainHandler = new Handler(Looper.getMainLooper());
    private volatile boolean running;
    private DatagramSocket socket;
    private WifiManager.MulticastLock multicastLock;

    SpanDiscovery(Context context, LocalIdentity identity, Listener listener) {
        this.context = context.getApplicationContext();
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
        releaseMulticastLock();
    }

    void destroy() {
        stop();
        executor.shutdownNow();
    }

    void announceOnce() {
        new Thread(() -> {
            sendProbe();
            sendAnnouncement();
        }, "span-announce").start();
    }

    private void runLoop() {
        long nextAnnounce = 0;
        try {
            acquireMulticastLock();
            socket = new DatagramSocket(null);
            socket.setReuseAddress(true);
            socket.setBroadcast(true);
            socket.bind(new InetSocketAddress(InetAddress.getByName("0.0.0.0"), SpanProtocol.DISCOVERY_PORT));
            socket.setSoTimeout(500);
            byte[] buffer = new byte[2048];
            while (running) {
                long now = System.currentTimeMillis();
                if (now >= nextAnnounce) {
                    sendProbe();
                    sendAnnouncement();
                    nextAnnounce = now + 15000;
                }
                try {
                    DatagramPacket packet = new DatagramPacket(buffer, buffer.length);
                    socket.receive(packet);
                    handlePacket(packet);
                } catch (java.net.SocketTimeoutException ignored) {
                } catch (Exception ignored) {
                }
            }
        } catch (Exception ignored) {
        } finally {
            if (socket != null) socket.close();
            releaseMulticastLock();
        }
    }

    private void handlePacket(DatagramPacket packet) {
        String value = new String(packet.getData(), 0, packet.getLength(), StandardCharsets.UTF_8);
        String host = packet.getAddress().getHostAddress();
        if (SpanProtocol.DISCOVERY_PROBE_MAGIC.equals(value.trim())) {
            sendAnnouncementTo(packet.getAddress(), packet.getPort());
            return;
        }
        SpanDevice device = parsePacket(value, host);
        if (device != null && !identity.id.equals(device.id)) {
            mainHandler.post(() -> listener.onDevice(device));
        }
    }

    private void sendProbe() {
        try (DatagramSocket out = new DatagramSocket()) {
            out.setBroadcast(true);
            byte[] data = SpanProtocol.DISCOVERY_PROBE_MAGIC.getBytes(StandardCharsets.UTF_8);
            sendToBroadcasts(out, data);
        } catch (Exception ignored) {
        }
    }

    private void sendAnnouncement() {
        try (DatagramSocket out = new DatagramSocket()) {
            out.setBroadcast(true);
            sendToBroadcasts(out, announcementBytes());
        } catch (Exception ignored) {
        }
    }

    private void sendAnnouncementTo(InetAddress address, int port) {
        try (DatagramSocket out = new DatagramSocket()) {
            byte[] data = announcementBytes();
            out.send(new DatagramPacket(data, data.length, address, port));
        } catch (Exception ignored) {
        }
    }

    private void sendToBroadcasts(DatagramSocket out, byte[] data) throws Exception {
        java.util.Enumeration<NetworkInterface> interfaces = NetworkInterface.getNetworkInterfaces();
        while (interfaces != null && interfaces.hasMoreElements()) {
            NetworkInterface networkInterface = interfaces.nextElement();
            if (!networkInterface.isUp() || networkInterface.isLoopback()) continue;
            for (InterfaceAddress address : networkInterface.getInterfaceAddresses()) {
                InetAddress broadcast = address.getBroadcast();
                if (broadcast == null) continue;
                out.send(new DatagramPacket(data, data.length, broadcast, SpanProtocol.DISCOVERY_PORT));
            }
        }
        // Some Android devices do not expose InterfaceAddress broadcast values.
        // Keep the global broadcast fallback so discovery still works on simple LANs.
        out.send(new DatagramPacket(data, data.length, InetAddress.getByName("255.255.255.255"), SpanProtocol.DISCOVERY_PORT));
    }

    private byte[] announcementBytes() {
        String payload = SpanProtocol.DISCOVERY_MAGIC + "\t" + identity.id + "\t" + sanitize(identity.name) + "\tandroid\t" + identity.publicKeyHex;
        return payload.getBytes(StandardCharsets.UTF_8);
    }

    private SpanDevice parsePacket(String value, String host) {
        String[] parts = value.trim().split("\t", -1);
        if (parts.length < 5 || !SpanProtocol.DISCOVERY_MAGIC.equals(parts[0])) return null;
        byte[] key = Hex.decode(parts[4]);
        if (key == null || key.length != 32) return null;
        return new SpanDevice(parts[1], parts[2], parts[3], host, parts[4], false, System.currentTimeMillis());
    }

    private void acquireMulticastLock() {
        try {
            WifiManager wifi = (WifiManager) context.getSystemService(Context.WIFI_SERVICE);
            if (wifi == null) return;
            multicastLock = wifi.createMulticastLock("span-discovery");
            multicastLock.setReferenceCounted(false);
            multicastLock.acquire();
        } catch (Exception ignored) {
        }
    }

    private void releaseMulticastLock() {
        try {
            if (multicastLock != null && multicastLock.isHeld()) multicastLock.release();
        } catch (Exception ignored) {
        } finally {
            multicastLock = null;
        }
    }

    private String sanitize(String s) {
        return s.replace('\t', ' ').replace('\n', ' ').replace('\r', ' ');
    }
}
