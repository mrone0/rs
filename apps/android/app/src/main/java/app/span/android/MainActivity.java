package app.span.android;

import android.Manifest;
import android.app.Activity;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.os.Bundle;
import android.text.InputType;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import android.widget.EditText;
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
    private EditText manualId;
    private EditText manualName;
    private EditText manualHost;
    private EditText manualKey;

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
        discovery.start();
        if (store.isReceiverEnabled()) SpanReceiveService.start(this);
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
        subtitle.setText("Fast local text flow across trusted devices");
        subtitle.setTextSize(14);
        root.addView(subtitle);

        status = new TextView(this);
        status.setText("Ready");
        status.setPadding(0, dp(12), 0, dp(12));
        root.addView(status);

        addSection(root, "Quick Send");
        Button sendClipboard = button("Send Clipboard");
        sendClipboard.setOnClickListener(v -> sendClipboard());
        root.addView(sendClipboard);

        addSection(root, "Network");
        Button start = button("Start Discovery");
        start.setOnClickListener(v -> { discovery.start(); setStatus("Discovery running"); });
        root.addView(start);
        Button announce = button("Announce Once");
        announce.setOnClickListener(v -> { discovery.announceOnce(); setStatus("Announced"); });
        root.addView(announce);
        Button stop = button("Stop Discovery");
        stop.setOnClickListener(v -> { discovery.stop(); setStatus("Discovery stopped"); });
        root.addView(stop);

        Button startReceiver = button("Start Receiver");
        startReceiver.setOnClickListener(v -> {
            store.setReceiverEnabled(true);
            SpanReceiveService.start(this);
            setStatus("Receiver running on TCP " + SpanProtocol.TEXT_PORT);
        });
        root.addView(startReceiver);
        Button stopReceiver = button("Stop Receiver");
        stopReceiver.setOnClickListener(v -> {
            store.setReceiverEnabled(false);
            SpanReceiveService.stop(this);
            setStatus("Receiver stopped");
        });
        root.addView(stopReceiver);

        addSection(root, "This Device");
        TextView local = new TextView(this);
        local.setText("id: " + identity.id + "\nplatform: android\npublic key: " + identity.publicKeyHex);
        local.setTextIsSelectable(true);
        local.setTextSize(12);
        root.addView(local);

        addSection(root, "Manual Pair");
        manualId = edit("Device ID");
        manualName = edit("Name");
        manualHost = edit("Host / IP");
        manualKey = edit("Public Key Hex");
        root.addView(manualId);
        root.addView(manualName);
        root.addView(manualHost);
        root.addView(manualKey);
        Button add = button("Trust Manual Device");
        add.setOnClickListener(v -> addManualDevice());
        root.addView(add);

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

    private void addManualDevice() {
        String id = manualId.getText().toString().trim();
        String name = manualName.getText().toString().trim();
        String host = manualHost.getText().toString().trim();
        String key = manualKey.getText().toString().trim().toLowerCase();
        byte[] keyBytes = Hex.decode(key);
        if (id.isEmpty() || host.isEmpty() || keyBytes == null || keyBytes.length != 32) {
            setStatus("Manual device needs id, host, and 32-byte public key");
            return;
        }
        store.upsertDiscovered(new SpanDevice(id, name.isEmpty() ? id : name, "unknown", host, key, true, System.currentTimeMillis()));
        manualId.setText(""); manualName.setText(""); manualHost.setText(""); manualKey.setText("");
        setStatus("Manual device trusted");
        refreshDevices();
    }

    private void refreshDevices() {
        devicesList.removeAllViews();
        List<SpanDevice> devices = store.loadDevices();
        if (devices.isEmpty()) {
            TextView empty = new TextView(this);
            empty.setText("No devices yet");
            devicesList.addView(empty);
            return;
        }
        for (SpanDevice device : devices) {
            LinearLayout row = new LinearLayout(this);
            row.setOrientation(LinearLayout.VERTICAL);
            row.setPadding(0, dp(8), 0, dp(8));
            TextView label = new TextView(this);
            label.setText(device.name + "  •  " + device.platform + "  •  " + (device.trusted ? "Trusted" : "Discovered") +
                    "\n" + (device.host == null ? "-" : device.host) +
                    "\nkey: " + (device.publicKeyHex == null ? "-" : device.publicKeyHex.substring(0, Math.min(16, device.publicKeyHex.length())) + "…"));
            row.addView(label);
            Button trust = button(device.trusted ? "Revoke" : "Trust");
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

    private EditText edit(String hint) {
        EditText e = new EditText(this);
        e.setHint(hint);
        e.setSingleLine(true);
        e.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS);
        return e;
    }

    private void setStatus(String text) { status.setText(text); }
    private int dp(int value) { return (int) (value * getResources().getDisplayMetrics().density + 0.5f); }
}
