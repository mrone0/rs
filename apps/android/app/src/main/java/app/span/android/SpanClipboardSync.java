package app.span.android;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.util.Log;
import java.nio.charset.StandardCharsets;

/** Coordinates local clipboard sends across the activity and foreground service. */
final class SpanClipboardSync {
    private static final String TAG = "SpanClipboardSync";
    private static final Object LOCK = new Object();
    private static final long DUPLICATE_WINDOW_MILLIS = 1500;
    private static final long REMOTE_ECHO_WINDOW_MILLIS = 5000;

    private static String inFlightText;
    private static String lastSentText;
    private static long lastSentAtMillis;
    private static String remoteText;
    private static long remoteTextUntilMillis;

    private SpanClipboardSync() {}

    static int sendCurrentClipboard(Context context) throws Exception {
        return sendText(context, readClipboardText(context), false);
    }

    static int sendSharedText(Context context, String text) throws Exception {
        return sendText(context, text, true);
    }

    static void markRemoteClipboard(String text) {
        synchronized (LOCK) {
            remoteText = text;
            remoteTextUntilMillis = System.currentTimeMillis() + REMOTE_ECHO_WINDOW_MILLIS;
        }
    }

    private static int sendText(Context context, String text, boolean explicitShare) throws Exception {
        if (text == null || text.trim().isEmpty()) return 0;
        if (text.getBytes(StandardCharsets.UTF_8).length > SpanProtocol.MAX_TEXT_BYTES) {
            throw new IllegalArgumentException("text too large");
        }
        SpanReceiveService.start(context);

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
