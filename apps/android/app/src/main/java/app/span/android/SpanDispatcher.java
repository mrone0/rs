package app.span.android;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import java.util.List;

final class SpanDispatcher {
    private final Context context;
    private final SpanStore store;
    private final SpanTransport transport = new SpanTransport();

    SpanDispatcher(Context context) {
        this.context = context.getApplicationContext();
        this.store = new SpanStore(this.context);
    }

    int sendClipboard() throws Exception {
        ClipboardManager cm = (ClipboardManager) context.getSystemService(Context.CLIPBOARD_SERVICE);
        if (cm == null || !cm.hasPrimaryClip()) return 0;
        ClipData clip = cm.getPrimaryClip();
        if (clip == null || clip.getItemCount() == 0) return 0;
        CharSequence text = clip.getItemAt(0).coerceToText(context);
        if (text == null) return 0;
        return sendText(text.toString());
    }

    int sendText(String text) throws Exception {
        LocalIdentity identity = store.loadOrCreateIdentity();
        List<SpanDevice> devices = store.loadDevices();
        int sent = 0;
        for (SpanDevice device : devices) {
            if (!device.trusted) continue;
            try {
                transport.sendText(text, identity, device);
                sent++;
            } catch (Exception ignored) {
                // Keep broadcasting to the remaining trusted devices. Stale IPs are
                // refreshed by discovery the next time the app sees an announcement.
            }
        }
        return sent;
    }
}
