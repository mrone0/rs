package app.span.android;

import android.Manifest;
import android.app.Activity;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.content.pm.ShortcutInfo;
import android.content.pm.ShortcutManager;
import android.graphics.drawable.Icon;
import android.os.Bundle;
import android.view.Gravity;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.Toast;
import java.util.List;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

public final class MainActivity extends Activity {
    static final String ACTION_SEND_CLIPBOARD = "app.span.android.action.SEND_CLIPBOARD";
    private SpanStore store;
    private LocalIdentity identity;
    private SpanDiscovery discovery;
    private final ExecutorService worker = Executors.newCachedThreadPool();
    private LinearLayout devicesList;
    private TextView status;

    @Override protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        store = new SpanStore(this);
        try {
            identity = store.loadOrCreateIdentity();
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
        discovery = new SpanDiscovery(this, identity, device -> {
            store.upsertDiscovered(device);
            setStatus("Found " + device.name);
            refreshDevices();
        });
        buildUi();
        installQuickSendShortcut();
        store.setReceiverEnabled(true);
        discovery.start();
        SpanReceiveService.start(this);
        requestNotificationPermission();
        setStatus("Discovery and receiver running");
        handleLaunchIntent(getIntent());
    }

    @Override protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        handleLaunchIntent(intent);
    }

    @Override protected void onDestroy() {
        discovery.destroy();
        worker.shutdownNow();
        super.onDestroy();
    }

    private void buildUi() {
        ScrollView scroll = new ScrollView(this);
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(dp(18), dp(18), dp(18), dp(18));
        scroll.addView(root);

        TextView title = new TextView(this);
        title.setText("Span");
        title.setTextSize(30);
        title.setGravity(Gravity.START);
        root.addView(title);

        TextView subtitle = new TextView(this);
        subtitle.setText("Copy here. Paste on trusted devices.");
        subtitle.setTextSize(14);
        root.addView(subtitle);

        status = new TextView(this);
        status.setText("Background receiver ready");
        status.setPadding(0, dp(12), 0, dp(12));
        root.addView(status);

        Button discover = button("Discover Devices");
        discover.setOnClickListener(v -> { discovery.announceOnce(); setStatus("Discovery requested"); });
        root.addView(discover);

        addSection(root, "This Device");
        TextView local = new TextView(this);
        local.setText("id: " + identity.id + "\nplatform: android\nkey: " + shortKey(identity.publicKeyHex));
        local.setTextIsSelectable(true);
        local.setTextSize(12);
        root.addView(local);

        addSection(root, "Devices");
        devicesList = new LinearLayout(this);
        devicesList.setOrientation(LinearLayout.VERTICAL);
        root.addView(devicesList);
        refreshDevices();

        setContentView(scroll);
    }

    private void requestNotificationPermission() {
        if (android.os.Build.VERSION.SDK_INT >= 33
                && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[]{Manifest.permission.POST_NOTIFICATIONS}, 1001);
        }
    }

    private void installQuickSendShortcut() {
        if (android.os.Build.VERSION.SDK_INT < android.os.Build.VERSION_CODES.N_MR1) return;
        Intent intent = new Intent(this, MainActivity.class)
                .setAction(ACTION_SEND_CLIPBOARD)
                .addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP);
        ShortcutInfo shortcut = new ShortcutInfo.Builder(this, "send-clipboard")
                .setShortLabel("Send clipboard")
                .setLongLabel("Wake Span and send clipboard")
                .setIcon(Icon.createWithResource(this, R.drawable.ic_span))
                .setIntent(intent)
                .build();
        ShortcutManager manager = getSystemService(ShortcutManager.class);
        if (manager != null) manager.setDynamicShortcuts(java.util.Collections.singletonList(shortcut));
    }

    private void handleLaunchIntent(Intent intent) {
        if (intent == null) return;
        if (ACTION_SEND_CLIPBOARD.equals(intent.getAction())) {
            sendClipboard(true);
            return;
        }
        if (Intent.ACTION_SEND.equals(intent.getAction()) && "text/plain".equals(intent.getType())) {
            String text = intent.getStringExtra(Intent.EXTRA_TEXT);
            if (text != null && !text.trim().isEmpty()) sendText(text, true);
        }
    }

    private void sendClipboard() { sendClipboard(false); }

    private void sendClipboard(boolean finishAfter) {
        worker.execute(() -> {
            try {
                int sent = new SpanDispatcher(this).sendClipboard();
                runOnUiThread(() -> {
                    setStatus(sent == 0 ? "Clipboard empty or no trusted devices" : "Sent to " + sent + " device(s)");
                    if (finishAfter) Toast.makeText(this, sent == 0 ? "Nothing sent" : "Sent with Span", Toast.LENGTH_SHORT).show();
                    if (finishAfter) finish();
                });
            } catch (Exception e) {
                runOnUiThread(() -> {
                    setStatus("Send failed: " + e.getClass().getSimpleName());
                    if (finishAfter) Toast.makeText(this, "Send failed", Toast.LENGTH_SHORT).show();
                });
            }
        });
    }

    private void sendText(String text, boolean finishAfter) {
        worker.execute(() -> {
            try {
                int sent = new SpanDispatcher(this).sendText(text);
                runOnUiThread(() -> {
                    setStatus(sent == 0 ? "No trusted devices" : "Sent to " + sent + " device(s)");
                    if (finishAfter) Toast.makeText(this, sent == 0 ? "No trusted devices" : "Sent with Span", Toast.LENGTH_SHORT).show();
                    if (finishAfter) finish();
                });
            } catch (Exception e) {
                runOnUiThread(() -> {
                    setStatus("Send failed: " + e.getClass().getSimpleName());
                    if (finishAfter) Toast.makeText(this, "Send failed", Toast.LENGTH_SHORT).show();
                });
            }
        });
    }

    private void refreshDevices() {
        devicesList.removeAllViews();
        List<SpanDevice> devices = store.loadDevices();
        if (devices.isEmpty()) {
            TextView empty = new TextView(this);
            empty.setText("No devices yet. Keep Span open on both devices, then tap Discover.");
            devicesList.addView(empty);
            return;
        }
        for (SpanDevice device : devices) {
            LinearLayout row = new LinearLayout(this);
            row.setOrientation(LinearLayout.VERTICAL);
            row.setPadding(0, dp(8), 0, dp(8));
            TextView label = new TextView(this);
            label.setText(device.name + "  -  " + device.platform + "  -  " + (device.trusted ? "Trusted" : "New") +
                    "\n" + (device.host == null ? "-" : device.host) + "  -  " + shortKey(device.publicKeyHex));
            row.addView(label);
            Button trust = button(device.trusted ? "Remove trusted device" : "Trust device");
            trust.setOnClickListener(v -> {
                store.setTrusted(device.id, !device.trusted);
                refreshDevices();
            });
            row.addView(trust);
            devicesList.addView(row);
        }
    }

    private void addSection(LinearLayout root, String text) {
        TextView tv = new TextView(this);
        tv.setText(text);
        tv.setTextSize(18);
        tv.setPadding(0, dp(22), 0, dp(8));
        root.addView(tv);
    }

    private Button button(String text) {
        Button b = new Button(this);
        b.setText(text);
        return b;
    }

    private String shortKey(String key) {
        if (key == null || key.length() < 16) return "key: -";
        return "key: " + key.substring(0, 16) + "…";
    }

    private void setStatus(String text) { status.setText(text); }
    private int dp(int value) { return (int) (value * getResources().getDisplayMetrics().density + 0.5f); }
}
