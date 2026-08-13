package app.span.android;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;
import android.widget.Toast;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

public final class SendClipboardActivity extends Activity {
    private final ExecutorService worker = Executors.newSingleThreadExecutor();

    @Override protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        SpanReceiveService.start(this);
        handleIntent(getIntent());
    }

    @Override protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        handleIntent(intent);
    }

    @Override protected void onDestroy() {
        worker.shutdownNow();
        super.onDestroy();
    }

    private void handleIntent(Intent intent) {
        String shared = null;
        if (intent != null && Intent.ACTION_SEND.equals(intent.getAction())
                && "text/plain".equals(intent.getType())) {
            shared = intent.getStringExtra(Intent.EXTRA_TEXT);
        }
        String explicitText = shared;
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
