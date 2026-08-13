package app.span.android;

import android.content.Context;
import android.util.Log;
import java.util.List;

final class SpanDispatcher {
    private static final String TAG = "SpanDispatcher";
    private final Context context;
    private final SpanStore store;
    private final SpanTransport transport = new SpanTransport();

    SpanDispatcher(Context context) {
        this.context = context.getApplicationContext();
        this.store = new SpanStore(this.context);
    }

    int sendText(String text) throws Exception {
        LocalIdentity identity = store.loadOrCreateIdentity();
        List<SpanDevice> devices = store.loadDevices();
        int sent = 0;
        int trusted = 0;
        Exception lastFailure = null;
        for (SpanDevice device : devices) {
            if (!device.trusted) continue;
            trusted++;
            try {
                transport.sendText(text, identity, device);
                sent++;
            } catch (Exception error) {
                // Keep broadcasting to the remaining trusted devices. Logging the
                // endpoint is essential for distinguishing clipboard restrictions
                // from stale discovery addresses or a desktop firewall.
                lastFailure = error;
                Log.w(TAG, "Send to " + device.name + " at " + device.host + " failed", error);
            }
        }
        if (trusted > 0 && sent == 0 && lastFailure != null) throw lastFailure;
        return sent;
    }
}
