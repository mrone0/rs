package app.span.android;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.widget.Toast;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicBoolean;

public final class SendClipboardActivity extends Activity {
    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private final Handler mainHandler = new Handler(Looper.getMainLooper());
    private final AtomicBoolean sendStarted = new AtomicBoolean();
    private String pendingExplicitText;

    @Override protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        SpanReceiveService.start(this);
        handleIntent(getIntent());
    }

    @Override protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        sendStarted.set(false);
        handleIntent(intent);
    }

    @Override public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus && !isFinishing()) {
            // Android 10+ only exposes the clipboard to the focused app. A tile
            // or notification action can create this Activity before its window
            // is focused, so never read the clipboard from onCreate directly.
            mainHandler.postDelayed(this::sendWhenFocused, 150);
        }
    }

    @Override protected void onDestroy() {
        mainHandler.removeCallbacksAndMessages(null);
        worker.shutdownNow();
        super.onDestroy();
    }

    private void handleIntent(Intent intent) {
        pendingExplicitText = null;
        if (intent != null && Intent.ACTION_SEND.equals(intent.getAction())
                && "text/plain".equals(intent.getType())) {
            pendingExplicitText = intent.getStringExtra(Intent.EXTRA_TEXT);
        }
        // Explicit shares do not need clipboard permission. Clipboard sends wait
        // for window focus so Quick Settings and notification actions work too.
        if (pendingExplicitText != null && !pendingExplicitText.trim().isEmpty()) {
            mainHandler.postDelayed(this::sendWhenFocused, 150);
        }
    }

    private void sendWhenFocused() {
        if (!hasWindowFocus() || isFinishing() || !sendStarted.compareAndSet(false, true)) return;
        String explicitText = pendingExplicitText;
        worker.execute(() -> sendAndFinish(explicitText));
    }

    private void sendAndFinish(String explicitText) {
        try {
            int sent = explicitText == null || explicitText.trim().isEmpty()
                    ? SpanClipboardSync.sendCurrentClipboard(this)
                    : SpanClipboardSync.sendSharedText(this, explicitText);
            runOnUiThread(() -> {
                Toast.makeText(
                        this,
                        sent == 0 ? "No trusted devices or empty clipboard" : "Sent to " + sent + " device(s)",
                        Toast.LENGTH_SHORT).show();
                finish();
            });
        } catch (SecurityException error) {
            runOnUiThread(() -> {
                Toast.makeText(this, "Android blocked clipboard access", Toast.LENGTH_SHORT).show();
                finish();
            });
        } catch (Exception error) {
            runOnUiThread(() -> {
                Toast.makeText(this, "Send failed", Toast.LENGTH_SHORT).show();
                finish();
            });
        }
    }
}
