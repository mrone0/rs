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
import android.net.wifi.WifiManager;
import android.os.Build;
import android.os.IBinder;
import android.os.PowerManager;
import android.util.Log;
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
    private static final String TAG = "SpanReceiveService";
    static final String ACTION_START = "app.span.android.action.START_RECEIVER";
    static final String ACTION_STOP = "app.span.android.action.STOP_RECEIVER";
    private static final String CHANNEL_ID = "span-transfer";
    private static final int NOTIFICATION_ID = 46793;
    private static final int RECEIVED_NOTIFICATION_ID = 46794;
    private static final int MAX_PACKET_BYTES = (SpanProtocol.MAX_TEXT_BYTES + 32) * 2 + 256;

    private final ExecutorService executor = Executors.newSingleThreadExecutor();
    private final ExecutorService sender = Executors.newSingleThreadExecutor();
    private volatile boolean running;
    private ServerSocket serverSocket;
    private LocalIdentity identity;
    private SpanStore store;
    private SpanDiscovery discovery;
    private ClipboardManager clipboard;
    private ClipboardManager.OnPrimaryClipChangedListener clipboardListener;
    private WifiManager.WifiLock wifiLock;
    private PowerManager.WakeLock wakeLock;

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
            Log.e(TAG, "Identity initialization failed", e);
            stopSelf();
        }
        if (identity != null) {
            discovery = new SpanDiscovery(this, identity, device -> store.upsertDiscovered(device));
        }
        clipboard = (ClipboardManager) getSystemService(CLIPBOARD_SERVICE);
        clipboardListener = this::onLocalClipboardChanged;
        createNotificationChannel();
    }

    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent != null && ACTION_STOP.equals(intent.getAction())) {
            stopSelf();
            return START_NOT_STICKY;
        }
        startForegroundCompat(buildListeningNotification());
        acquireKeepAliveLocks();
        startClipboardWatcher();
        startDiscovery();
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
        stopDiscovery();
        stopClipboardWatcher();
        releaseKeepAliveLocks();
        executor.shutdownNow();
        sender.shutdownNow();
        super.onDestroy();
    }

    @Override public void onTaskRemoved(Intent rootIntent) {
        if (store != null && store.isReceiverEnabled()) SpanReceiveService.start(this);
        super.onTaskRemoved(rootIntent);
    }

    @Override public IBinder onBind(Intent intent) { return null; }

    private void startDiscovery() {
        try {
            if (discovery != null) discovery.start();
        } catch (Exception ignored) {
        }
    }

    private void stopDiscovery() {
        try {
            if (discovery != null) discovery.destroy();
        } catch (Exception ignored) {
        }
    }

    private void acquireKeepAliveLocks() {
        try {
            if (wifiLock == null) {
                WifiManager wifi = (WifiManager) getApplicationContext().getSystemService(WIFI_SERVICE);
                if (wifi != null) {
                    wifiLock = wifi.createWifiLock(WifiManager.WIFI_MODE_FULL_LOW_LATENCY, "Span:receiver-wifi");
                    wifiLock.setReferenceCounted(false);
                }
            }
            if (wifiLock != null && !wifiLock.isHeld()) wifiLock.acquire();
        } catch (Exception error) {
            Log.w(TAG, "Wi-Fi keepalive lock unavailable", error);
        }

        try {
            if (wakeLock == null) {
                PowerManager power = (PowerManager) getSystemService(POWER_SERVICE);
                if (power != null) {
                    wakeLock = power.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "Span:receiver-cpu");
                    wakeLock.setReferenceCounted(false);
                }
            }
            if (wakeLock != null && !wakeLock.isHeld()) wakeLock.acquire();
        } catch (Exception error) {
            Log.w(TAG, "CPU keepalive lock unavailable", error);
        }
    }

    private void releaseKeepAliveLocks() {
        try {
            if (wifiLock != null && wifiLock.isHeld()) wifiLock.release();
        } catch (Exception ignored) {
        }
        try {
            if (wakeLock != null && wakeLock.isHeld()) wakeLock.release();
        } catch (Exception ignored) {
        }
    }

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
                    } catch (Exception error) {
                        if (!running) break;
                        Log.e(TAG, "Client receive failed", error);
                    }
                }
            } catch (Exception error) {
                if (running) {
                    Log.e(TAG, "Listen failed; retrying", error);
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
        if (packet == null || identity == null || store == null) {
            Log.w(TAG, "Malformed packet rejected");
            return;
        }

        SpanDevice sender = store.trustedDevice(packet.fromDeviceId);
        if (sender == null || sender.publicKeyHex == null || sender.publicKeyHex.trim().isEmpty()) {
            Log.w(TAG, "Packet rejected from untrusted sender");
            return;
        }

        try {
            String text = SpanCrypto.decryptText(packet, identity.privateKeyHex, sender.publicKeyHex);
            byte[] utf8 = text.getBytes(StandardCharsets.UTF_8);
            if (utf8.length > SpanProtocol.MAX_TEXT_BYTES) return;
            ClipboardManager clipboard = (ClipboardManager) getSystemService(CLIPBOARD_SERVICE);
            if (clipboard == null) return;
            SpanClipboardSync.markRemoteClipboard(this, text);
            clipboard.setPrimaryClip(ClipData.newPlainText("Span", text));
            notifyReceived(sender.name, text);
        } catch (Exception error) {
            Log.e(TAG, "Decrypt or clipboard update failed", error);
        }
    }

    private void startClipboardWatcher() {
        try {
            if (clipboard != null && clipboardListener != null) {
                clipboard.removePrimaryClipChangedListener(clipboardListener);
                clipboard.addPrimaryClipChangedListener(clipboardListener);
            }
        } catch (Exception ignored) {
        }
    }

    private void stopClipboardWatcher() {
        try {
            if (clipboard != null && clipboardListener != null) {
                clipboard.removePrimaryClipChangedListener(clipboardListener);
            }
        } catch (Exception ignored) {
        }
    }

    private void onLocalClipboardChanged() {
        sender.execute(() -> {
            try {
                int sent = SpanClipboardSync.sendCurrentClipboard(this);
                Log.d(TAG, "Clipboard change sent to " + sent + " trusted device(s)");
            } catch (SecurityException error) {
                // Android 10+ commonly reports the change to a background app but
                // denies reading its value. MainActivity retries after it gains
                // window focus, which is the earliest reliable access point.
                Log.w(TAG, "Background clipboard read denied; waiting for foreground wake", error);
            } catch (Exception error) {
                Log.e(TAG, "Clipboard send failed", error);
            }
        });
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
        Intent send = new Intent(this, SendClipboardActivity.class);
        send.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP);
        PendingIntent sendPending = PendingIntent.getActivity(
                this, 1, send,
                PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        return new Notification.Builder(this, CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_span)
                .setContentTitle("Span")
                .setContentText("Receiving from PC in background")
                .setOngoing(true)
                .setContentIntent(pending)
                .addAction(R.drawable.ic_span, "Send clipboard", sendPending)
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
