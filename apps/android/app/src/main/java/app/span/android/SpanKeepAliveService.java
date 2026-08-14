package app.span.android;

import android.accessibilityservice.AccessibilityService;
import android.accessibilityservice.AccessibilityServiceInfo;
import android.content.Context;
import android.os.Handler;
import android.os.Looper;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityManager;
import java.util.List;

/**
 * Optional system-bound watchdog for vendors that kill normal foreground services.
 *
 * It requests no accessibility events, reads no screen content, and performs no
 * gestures. Android keeps the service binding after the user enables it; that
 * binding is used only to restore Span's tiny LAN receiver when needed.
 */
public final class SpanKeepAliveService extends AccessibilityService {
    private static final long HEARTBEAT_MILLIS = 60_000;
    private final Handler handler = new Handler(Looper.getMainLooper());
    private final Runnable heartbeat = new Runnable() {
        @Override public void run() {
            ensureReceiver();
            handler.postDelayed(this, HEARTBEAT_MILLIS);
        }
    };

    @Override protected void onServiceConnected() {
        super.onServiceConnected();
        handler.removeCallbacks(heartbeat);
        ensureReceiver();
        handler.postDelayed(heartbeat, HEARTBEAT_MILLIS);
    }

    @Override public void onAccessibilityEvent(AccessibilityEvent event) {
        // Intentionally empty: Span does not inspect apps, windows, or controls.
    }

    @Override public void onInterrupt() {
        ensureReceiver();
    }

    @Override public void onDestroy() {
        handler.removeCallbacks(heartbeat);
        super.onDestroy();
    }

    private void ensureReceiver() {
        if (!new SpanStore(this).isReceiverEnabled()) return;
        if (!SpanReceiveService.isRunning()) SpanReceiveService.start(this);
        // A pending item only remains when a previous platform clipboard call
        // threw. Retry it after Android rebinds this watchdog.
        SpanClipboardSync.writePendingRemoteClipboard(this);
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
