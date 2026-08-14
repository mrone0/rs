package app.span.android;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;

public final class SpanBootReceiver extends BroadcastReceiver {
    @Override public void onReceive(Context context, Intent intent) {
        String action = intent == null ? null : intent.getAction();
        boolean shouldRestore = Intent.ACTION_BOOT_COMPLETED.equals(action)
                || Intent.ACTION_MY_PACKAGE_REPLACED.equals(action)
                || "android.intent.action.QUICKBOOT_POWERON".equals(action);
        if (!shouldRestore) return;
        SpanStore store = new SpanStore(context);
        if (store.isReceiverEnabled()) SpanReceiveService.start(context);
    }
}
