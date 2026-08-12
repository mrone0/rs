package app.span.android;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;

public final class SpanBootReceiver extends BroadcastReceiver {
    @Override public void onReceive(Context context, Intent intent) {
        if (!Intent.ACTION_BOOT_COMPLETED.equals(intent.getAction())) return;
        SpanStore store = new SpanStore(context);
        if (store.isReceiverEnabled()) SpanReceiveService.start(context);
    }
}
