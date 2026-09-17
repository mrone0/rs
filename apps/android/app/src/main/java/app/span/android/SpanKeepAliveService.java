package app.span.android;

import android.accessibilityservice.AccessibilityButtonController;
import android.accessibilityservice.AccessibilityService;
import android.accessibilityservice.AccessibilityServiceInfo;
import android.content.Context;
import android.os.Handler;
import android.os.Looper;
import android.view.accessibility.AccessibilityEvent;
import java.lang.ref.WeakReference;
import android.view.accessibility.AccessibilityManager;
import android.widget.Toast;
import java.util.List;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * Optional system-bound watchdog for vendors that kill normal foreground services.
 *
 * It reads no screen content and performs no gestures. Android keeps the
 * service binding after the user enables it; that binding is used to restore
 * Span's tiny LAN receiver and retry a deferred PC clipboard write when the
 * user switches into another app to paste.
 */
public final class SpanKeepAliveService extends AccessibilityService {
    private static final long HEARTBEAT_MILLIS = 60_000;
    private static final long EVENT_RETRY_DEBOUNCE_MILLIS = 500;
    private static WeakReference<SpanKeepAliveService> activeService = new WeakReference<>(null);
    private final Handler handler = new Handler(Looper.getMainLooper());
    private final ExecutorService sendWorker = Executors.newSingleThreadExecutor();
    private final AtomicBoolean sendInProgress = new AtomicBoolean();
    private long lastEventRetryMillis;
    private AccessibilityButtonController accessibilityButtonController;
    private final AccessibilityButtonController.AccessibilityButtonCallback accessibilityButtonCallback =
            new AccessibilityButtonController.AccessibilityButtonCallback() {
                @Override public void onClicked(AccessibilityButtonController controller) {
                    sendClipboardWithoutOpeningApp();
                }
            };
    private final Runnable heartbeat = new Runnable() {
        @Override public void run() {
            ensureReceiver();
            handler.postDelayed(this, HEARTBEAT_MILLIS);
        }
    };

    @Override protected void onServiceConnected() {
        super.onServiceConnected();
        activeService = new WeakReference<>(this);
        accessibilityButtonController = getAccessibilityButtonController();
        accessibilityButtonController.registerAccessibilityButtonCallback(
                accessibilityButtonCallback, handler);
        handler.removeCallbacks(heartbeat);
        ensureReceiver();
        handler.postDelayed(heartbeat, HEARTBEAT_MILLIS);
    }

    @Override public void onAccessibilityEvent(AccessibilityEvent event) {
        // Do not inspect event contents. A foreground app switch is enough to
        // retry a pending PC clipboard write before the user long-presses Paste.
        int type = event == null ? 0 : event.getEventType();
        if (type != AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED
                && type != AccessibilityEvent.TYPE_WINDOWS_CHANGED) {
            return;
        }
        long now = System.currentTimeMillis();
        if (now - lastEventRetryMillis < EVENT_RETRY_DEBOUNCE_MILLIS) return;
        lastEventRetryMillis = now;
        ensureReceiver();
    }

    @Override public void onInterrupt() {
        ensureReceiver();
    }

    @Override public void onDestroy() {
        handler.removeCallbacks(heartbeat);
        sendWorker.shutdownNow();
        if (accessibilityButtonController != null) {
            accessibilityButtonController.unregisterAccessibilityButtonCallback(
                    accessibilityButtonCallback);
            accessibilityButtonController = null;
        }
        SpanKeepAliveService current = activeService.get();
        if (current == this) activeService = new WeakReference<>(null);
        super.onDestroy();
    }

    private void sendClipboardWithoutOpeningApp() {
        if (!sendInProgress.compareAndSet(false, true)) return;
        sendWorker.execute(() -> {
            String message;
            try {
                boolean trusted = false;
                for (SpanDevice device : new SpanStore(this).loadDevices()) {
                    if (device.trusted) { trusted = true; break; }
                }
                if (!trusted) {
                    message = "尚未连接可信设备，请先完成配对";
                } else {
                    int sent = SpanClipboardSync.sendCurrentClipboard(this);
                    message = sent > 0
                            ? "已发送到 " + sent + " 台设备"
                            : "发送已在处理中，或当前内容不是可发送的文本";
                }
            } catch (SecurityException error) {
                message = "无法从后台读取剪贴板；请用系统分享菜单发送文本";
            } catch (Exception error) {
                message = "发送失败，请检查电脑是否在线";
            } finally {
                sendInProgress.set(false);
            }
            String result = message;
            handler.post(() -> Toast.makeText(
                    SpanKeepAliveService.this, result, Toast.LENGTH_SHORT).show());
        });
    }

    private void ensureReceiver() {
        if (!new SpanStore(this).isReceiverEnabled()) return;
        if (!SpanReceiveService.isRunning()) SpanReceiveService.start(this);
        // A pending item only remains when a previous platform clipboard call
        // threw. Retry it after Android rebinds this watchdog.
        SpanClipboardSync.writePendingRemoteClipboard(this);
    }

    static boolean requestClipboardRetry() {
        SpanKeepAliveService service = activeService.get();
        if (service == null) return false;
        service.handler.post(service::ensureReceiver);
        return true;
    }

    static boolean isEnabled(Context context) {
        AccessibilityManager manager =
                (AccessibilityManager) context.getSystemService(Context.ACCESSIBILITY_SERVICE);
        if (manager == null) return false;
        List<android.accessibilityservice.AccessibilityServiceInfo> enabled =
                manager.getEnabledAccessibilityServiceList(AccessibilityServiceInfo.FEEDBACK_ALL_MASK);
        String packageName = context.getPackageName();
        String className = SpanKeepAliveService.class.getName();
        for (android.accessibilityservice.AccessibilityServiceInfo info : enabled) {
            if (info.getResolveInfo() == null || info.getResolveInfo().serviceInfo == null) continue;
            android.content.pm.ServiceInfo service = info.getResolveInfo().serviceInfo;
            if (packageName.equals(service.packageName) && className.equals(service.name)) return true;
        }
        return false;
    }
}
