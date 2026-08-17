package app.span.android;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.util.Log;
import java.nio.charset.StandardCharsets;

/** Coordinates local clipboard sends across the activity and foreground service. */
final class SpanClipboardSync {
    private static final String TAG = "SpanClipboardSync";
    private static final String PREFS = "span";
    private static final String PENDING_REMOTE_TEXT = "clipboard.pending_remote_text";
    private static final Object LOCK = new Object();
    private static final long DUPLICATE_WINDOW_MILLIS = 1500;
    private static final long REMOTE_ECHO_WINDOW_MILLIS = 5000;

    private static String inFlightText;
    private static String lastSentText;
    private static long lastSentAtMillis;
    private static String remoteText;
    private static long remoteTextUntilMillis;
    private static String pendingRemoteText;

    private SpanClipboardSync() {}

    static int sendCurrentClipboard(Context context) throws Exception {
        return sendText(context, readClipboardText(context), false);
    }

    static int sendSharedText(Context context, String text) throws Exception {
        return sendText(context, text, true);
    }

    static void markRemoteClipboard(Context context, String text) {
        synchronized (LOCK) {
            remoteText = text;
            pendingRemoteText = text;
            remoteTextUntilMillis = System.currentTimeMillis() + REMOTE_ECHO_WINDOW_MILLIS;
        }
        context.getApplicationContext()
                .getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                .edit()
                .putString(PENDING_REMOTE_TEXT, text)
                .apply();
    }

    static boolean writePendingRemoteClipboard(Context context) {
        String text;
        synchronized (LOCK) {
            text = pendingRemoteText;
        }
        if (text == null) {
            text = context.getApplicationContext()
                    .getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                    .getString(PENDING_REMOTE_TEXT, null);
        }
        if (text == null || text.isEmpty()) return false;

        ClipboardManager clipboard =
                (ClipboardManager) context.getSystemService(Context.CLIPBOARD_SERVICE);
        if (clipboard == null) return false;
        try {
            clipboard.setPrimaryClip(ClipData.newPlainText("Span", text));
        } catch (RuntimeException error) {
            // Keep the persisted pending value. The foreground Activity or the
            // system-bound watchdog can retry when Android permits the call.
            Log.w(TAG, "System clipboard write deferred from "
                    + context.getClass().getSimpleName(), error);
            return false;
        }

        synchronized (LOCK) {
            if (text.equals(pendingRemoteText)) pendingRemoteText = null;
        }
        context.getApplicationContext()
                .getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                .edit()
                .remove(PENDING_REMOTE_TEXT)
                .apply();
        return true;
    }


    private static int sendText(Context context, String text, boolean explicitShare) throws Exception {
        if (text == null || text.trim().isEmpty()) return 0;
        if (text.getBytes(StandardCharsets.UTF_8).length > SpanProtocol.MAX_TEXT_BYTES) {
            throw new IllegalArgumentException("text too large");
        }
        long now = System.currentTimeMillis();
        synchronized (LOCK) {
            if (!explicitShare && remoteText != null && remoteText.equals(text)
                    && now <= remoteTextUntilMillis) {
                Log.d(TAG, "Skipped clipboard text received from a remote device");
                return 0;
            }
            if (inFlightText != null && inFlightText.equals(text)) return 0;
            if (lastSentText != null && lastSentText.equals(text)
                    && now - lastSentAtMillis <= DUPLICATE_WINDOW_MILLIS) {
                return 0;
            }
            inFlightText = text;
        }

        int sent = 0;
        try {
            sent = new SpanDispatcher(context).sendText(text);
            return sent;
        } finally {
            synchronized (LOCK) {
                if (text.equals(inFlightText)) inFlightText = null;
                if (sent > 0) {
                    lastSentText = text;
                    lastSentAtMillis = System.currentTimeMillis();
                }
            }
        }
    }

    private static String readClipboardText(Context context) {
        Context app = context.getApplicationContext();
        ClipboardManager clipboard =
                (ClipboardManager) app.getSystemService(Context.CLIPBOARD_SERVICE);
        if (clipboard == null || !clipboard.hasPrimaryClip()) return null;
        ClipData clip = clipboard.getPrimaryClip();
        if (clip == null || clip.getItemCount() == 0) return null;
        CharSequence text = clip.getItemAt(0).coerceToText(app);
        return text == null ? null : text.toString();
    }
}
