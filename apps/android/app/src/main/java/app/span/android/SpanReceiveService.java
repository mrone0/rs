package app.span.android;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.os.Build;
import android.os.IBinder;
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
    private volatile boolean running;
    private ServerSocket serverSocket;
    private LocalIdentity identity;
    private SpanStore store;
    private SpanDiscovery discovery;
    private static volatile boolean serviceRunning;

    static boolean isRunning() { return serviceRunning; }

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
        serviceRunning = true;
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
        createNotificationChannel();
    }

    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent != null && ACTION_STOP.equals(intent.getAction())) {
            stopSelf();
            return START_NOT_STICKY;
        }
        startForegroundCompat(buildListeningNotification());
        startDiscovery();
        if (!running && identity != null) {
            running = true;
            executor.execute(this::listenLoop);
        }
        return START_STICKY;
    }

    @Override public void onDestroy() {
        serviceRunning = false;
        running = false;
        if (serverSocket != null) {
            try { serverSocket.close(); } catch (Exception ignored) {}
        }
        stopDiscovery();
        executor.shutdownNow();
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
        Log.i(TAG, "Packet received from " + shortId(packet.fromDeviceId));

        SpanDevice sender = store.trustedDevice(packet.fromDeviceId);
        if (sender == null || sender.publicKeyHex == null || sender.publicKeyHex.trim().isEmpty()) {
            Log.w(TAG, "Packet rejected from untrusted sender " + shortId(packet.fromDeviceId));
            return;
        }

        try {
            String text = SpanCrypto.decryptText(packet, identity.privateKeyHex, sender.publicKeyHex);
            byte[] utf8 = text.getBytes(StandardCharsets.UTF_8);
            if (utf8.length > SpanProtocol.MAX_TEXT_BYTES) {
                Log.w(TAG, "Decrypted text exceeds limit from " + shortId(packet.fromDeviceId));
                return;
            }
            Log.i(TAG, "Trusted text decrypted from " + shortId(packet.fromDeviceId)
                    + " bytes=" + utf8.length);
            SpanClipboardSync.markRemoteClipboard(this, text);
            boolean written = SpanClipboardSync.writePendingRemoteClipboard(this);
            if (!written && SpanKeepAliveService.requestClipboardRetry()) {
                Log.i(TAG, "System clipboard write handed to accessibility watchdog");
            } else {
                Log.i(TAG, "System clipboard write " + (written ? "completed" : "deferred"));
            }
            notifyReceived(sender.name, text, written);
        } catch (Exception error) {
            // Never log clipboard contents. Sender ID and the exception are enough
            // to distinguish trust, crypto and platform clipboard failures.
            Log.e(TAG, "Decrypt or clipboard update failed from "
                    + shortId(packet.fromDeviceId), error);
        }
    }

    private static String shortId(String id) {
        if (id == null || id.isEmpty()) return "-";
        return id.length() <= 12 ? id : id.substring(0, 12) + "…";
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
                .setContentTitle("Span 后台同步")
                .setContentText("正在接收可信设备发送的文本")
                .setOngoing(true)
                .setContentIntent(pending)
                .addAction(R.drawable.ic_span, "发送剪贴板", sendPending)
                .build();
    }

    private void notifyReceived(String senderName, String text, boolean written) {
        Notification notification = new Notification.Builder(this, CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_span)
                .setContentTitle("收到来自 " + senderName + " 的文本")
                .setContentText(written ? "已写入剪贴板，可以直接粘贴" : "等待系统允许写入剪贴板")
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
                CHANNEL_ID, "Span 剪贴板同步", NotificationManager.IMPORTANCE_LOW);
        channel.setDescription("在可信设备之间接收和发送剪贴板文本");
        manager.createNotificationChannel(channel);
    }
}
