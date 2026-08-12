package app.span.android;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.os.Build;
import android.os.IBinder;
import java.io.BufferedInputStream;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.net.InetSocketAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

public final class SpanReceiveService extends Service {
    static final String ACTION_START = "app.span.android.action.START_RECEIVER";
    static final String ACTION_STOP = "app.span.android.action.STOP_RECEIVER";
    private static final String CHANNEL_ID = "span-transfer";
    private static final int NOTIFICATION_ID = 46793;
    private static final int RECEIVED_NOTIFICATION_ID = 46794;
    private static final int MAX_PACKET_BYTES = (SpanProtocol.MAX_TEXT_BYTES + 32) * 2 + 256;

    private final ExecutorService executor = Executors.newSingleThreadExecutor();
    private volatile boolean running;
    private ServerSocket serverSocket;
    private LocalIdentity identity;
    private SpanStore store;

    static void start(Context context) {
        Intent intent = new Intent(context, SpanReceiveService.class).setAction(ACTION_START);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            context.startForegroundService(intent);
        } else {
            context.startService(intent);
        }
    }

    static void stop(Context context) {
        context.startService(new Intent(context, SpanReceiveService.class).setAction(ACTION_STOP));
    }

    @Override public void onCreate() {
        super.onCreate();
        store = new SpanStore(this);
        try {
            identity = store.loadOrCreateIdentity();
        } catch (Exception e) {
            stopSelf();
        }
        createNotificationChannel();
    }

    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent != null && ACTION_STOP.equals(intent.getAction())) {
            stopSelf();
            return START_NOT_STICKY;
        }
        startForegroundCompat(buildListeningNotification());
        if (!running && identity != null) {
            running = true;
            executor.execute(this::listenLoop);
        }
        return START_STICKY;
    }

    @Override public void onDestroy() {
        running = false;
        if (serverSocket != null) {
            try { serverSocket.close(); } catch (Exception ignored) {}
        }
        executor.shutdownNow();
        super.onDestroy();
    }

    @Override public IBinder onBind(Intent intent) { return null; }

    private void listenLoop() {
        while (running) {
            try {
                ServerSocket socket = new ServerSocket();
                socket.setReuseAddress(true);
                socket.bind(new InetSocketAddress("0.0.0.0", SpanProtocol.TEXT_PORT), 16);
                serverSocket = socket;
                while (running) {
                    try (Socket client = socket.accept()) {
                        client.setSoTimeout(5000);
                        String line = readLineBounded(client.getInputStream());
                        if (line != null) receiveLine(line);
                    } catch (Exception ignored) {
                        if (!running) break;
                    }
                }
            } catch (Exception ignored) {
                if (running) {
                    try { Thread.sleep(1000); } catch (InterruptedException interrupted) {
                        Thread.currentThread().interrupt();
                        break;
                    }
                }
            } finally {
                if (serverSocket != null) {
                    try { serverSocket.close(); } catch (Exception ignored) {}
                    serverSocket = null;
                }
            }
        }
    }

    private void receiveLine(String line) {
        SpanTextPacket packet = SpanTextPacket.parse(line);
        if (packet == null || identity == null || store == null) return;

        SpanDevice sender = store.trustedDevice(packet.fromDeviceId);
        if (sender == null || sender.publicKeyHex == null || sender.publicKeyHex.trim().isEmpty()) return;

        try {
            String text = SpanCrypto.decryptText(packet, identity.privateKeyHex, sender.publicKeyHex);
            byte[] utf8 = text.getBytes(StandardCharsets.UTF_8);
            if (utf8.length > SpanProtocol.MAX_TEXT_BYTES) return;
            ClipboardManager clipboard = (ClipboardManager) getSystemService(CLIPBOARD_SERVICE);
            if (clipboard == null) return;
            clipboard.setPrimaryClip(ClipData.newPlainText("Span", text));
            notifyReceived(sender.name, text);
        } catch (Exception ignored) {
            // Invalid authentication, malformed data, or a revoked key is discarded.
        }
    }

    private String readLineBounded(InputStream input) throws Exception {
        BufferedInputStream buffered = new BufferedInputStream(input);
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        int value;
        while ((value = buffered.read()) != -1) {
            if (value == '\n') break;
            if (value == '\r') continue;
            if (output.size() >= MAX_PACKET_BYTES) return null;
            output.write(value);
        }
        if (output.size() == 0) return null;
        return output.toString(StandardCharsets.UTF_8.name());
    }

    private void startForegroundCompat(Notification notification) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE);
        } else {
            startForeground(NOTIFICATION_ID, notification);
        }
    }

    private Notification buildListeningNotification() {
        Intent open = new Intent(this, MainActivity.class);
        PendingIntent pending = PendingIntent.getActivity(
                this, 0, open,
                PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        return new Notification.Builder(this, CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_span)
                .setContentTitle("Span")
                .setContentText("Ready to receive trusted text")
                .setOngoing(true)
                .setContentIntent(pending)
                .build();
    }

    private void notifyReceived(String senderName, String text) {
        String preview = text.replace('\n', ' ').trim();
        if (preview.length() > 80) preview = preview.substring(0, 80) + "…";
        Notification notification = new Notification.Builder(this, CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_span)
                .setContentTitle("Text received from " + senderName)
                .setContentText(preview.isEmpty() ? "Clipboard updated" : preview)
                .setAutoCancel(true)
                .build();
        NotificationManager manager = (NotificationManager) getSystemService(NOTIFICATION_SERVICE);
        if (manager != null) manager.notify(RECEIVED_NOTIFICATION_ID, notification);
    }

    private void createNotificationChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return;
        NotificationManager manager = (NotificationManager) getSystemService(NOTIFICATION_SERVICE);
        if (manager == null) return;
        NotificationChannel channel = new NotificationChannel(
                CHANNEL_ID, "Span transfer", NotificationManager.IMPORTANCE_LOW);
        channel.setDescription("Span trusted-device clipboard transfer");
        manager.createNotificationChannel(channel);
    }
}
