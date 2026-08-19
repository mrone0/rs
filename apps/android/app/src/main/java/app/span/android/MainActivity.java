package app.span.android;

import android.Manifest;
import android.app.Activity;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.net.Uri;
import android.os.Bundle;
import android.os.PowerManager;
import android.provider.Settings;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.Toast;
import java.util.List;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** Span 的轻量中文首页：状态、发送、后台设置和可信设备管理。 */
public final class MainActivity extends Activity {
    private static final int BLUE = Color.rgb(38, 104, 242);
    private static final int BLUE_SOFT = Color.rgb(235, 242, 255);
    private static final int GREEN = Color.rgb(31, 153, 91);
    private static final int GREEN_SOFT = Color.rgb(232, 247, 239);
    private static final int ORANGE = Color.rgb(210, 126, 24);
    private static final int ORANGE_SOFT = Color.rgb(255, 246, 230);
    private static final int TEXT = Color.rgb(28, 32, 39);
    private static final int TEXT_MUTED = Color.rgb(104, 113, 128);
    private static final int SURFACE = Color.WHITE;
    private static final int PAGE = Color.rgb(245, 247, 250);
    private static final int DIVIDER = Color.rgb(229, 233, 239);

    private SpanStore store;
    private LocalIdentity identity;
    private SpanDiscovery discovery;
    private final ExecutorService worker = Executors.newCachedThreadPool();
    private LinearLayout devicesList;
    private TextView statusTitle;
    private TextView statusDetail;
    private TextView deviceSectionTitle;
    private TextView backgroundTitle;
    private TextView backgroundDetail;
    private Button backgroundButton;
    private Button sendButton;

    @Override protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        store = new SpanStore(this);
        try {
            identity = store.loadOrCreateIdentity();
        } catch (Exception error) {
            throw new RuntimeException(error);
        }
        discovery = new SpanDiscovery(this, identity, device -> {
            store.upsertDiscovered(device);
            runOnUiThread(() -> {
                setStatus("发现设备", "已发现 " + device.name + "，请确认是否信任");
                refreshDevices();
            });
        });
        buildUi();
        store.setReceiverEnabled(true);
        discovery.start();
        SpanReceiveService.start(this);
        requestNotificationPermission();
        setStatus("同步服务运行中", "已在后台接收可信设备发送的文本");
        handleLaunchIntent(getIntent());
    }

    @Override protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        handleLaunchIntent(intent);
    }

    @Override protected void onResume() {
        super.onResume();
        updateReliableBackgroundState();
        refreshDevices();
        if (!isFinishing()) SpanClipboardSync.writePendingRemoteClipboard(this);
    }

    @Override public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (!hasFocus || isFinishing() || worker.isShutdown()) return;
        if (SpanClipboardSync.writePendingRemoteClipboard(this)) return;
        // 保留原有体验：通过启动器打开 Span 时，窗口获得焦点后自动发送。
        // 快速按钮和通知入口仍使用 SendClipboardActivity，避免依赖首页。
        worker.execute(this::sendClipboardAfterWake);
    }

    @Override protected void onDestroy() {
        discovery.destroy();
        worker.shutdownNow();
        super.onDestroy();
    }

    private void buildUi() {
        getWindow().setStatusBarColor(PAGE);
        getWindow().setNavigationBarColor(PAGE);

        ScrollView scroll = new ScrollView(this);
        scroll.setFillViewport(true);
        scroll.setBackgroundColor(PAGE);
        LinearLayout root = column();
        root.setPadding(dp(20), dp(18), dp(20), dp(32));
        scroll.addView(root, matchWrap());

        TextView title = text("Span", 30, TEXT, Typeface.BOLD);
        root.addView(title);
        TextView subtitle = text("跨设备剪贴板", 15, TEXT_MUTED, Typeface.NORMAL);
        subtitle.setPadding(0, dp(3), 0, dp(18));
        root.addView(subtitle);

        LinearLayout statusCard = card(GREEN_SOFT);
        LinearLayout statusHead = row();
        TextView dot = text("●", 14, GREEN, Typeface.BOLD);
        dot.setGravity(Gravity.CENTER_VERTICAL);
        statusHead.addView(dot, new LinearLayout.LayoutParams(dp(24), dp(28)));
        LinearLayout statusCopy = column();
        statusTitle = text("正在启动", 17, TEXT, Typeface.BOLD);
        statusDetail = text("正在准备局域网接收服务", 13, TEXT_MUTED, Typeface.NORMAL);
        statusDetail.setPadding(0, dp(3), 0, 0);
        statusCopy.addView(statusTitle);
        statusCopy.addView(statusDetail);
        statusHead.addView(statusCopy, new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        statusCard.addView(statusHead);
        root.addView(statusCard, spacedCard());

        addSection(root, "快速操作", "复制后点一下即可发送到所有可信设备");
        LinearLayout actionCard = card(SURFACE);
        sendButton = primaryButton("发送当前剪贴板");
        sendButton.setOnClickListener(v -> sendCurrentClipboard());
        actionCard.addView(sendButton, matchHeight(dp(50)));
        Button discover = secondaryButton("刷新附近设备");
        LinearLayout.LayoutParams discoverParams = matchHeight(dp(48));
        discoverParams.topMargin = dp(10);
        actionCard.addView(discover, discoverParams);
        discover.setOnClickListener(v -> {
            discovery.announceOnce();
            setStatus("正在发现设备", "请让另一台设备保持 Span 运行并连接同一 Wi-Fi");
        });
        root.addView(actionCard, spacedCard());

        addSection(root, "后台同步", "确保锁屏或切换应用后仍能接收");
        LinearLayout backgroundCard = card(SURFACE);
        backgroundTitle = text("检查后台能力", 17, TEXT, Typeface.BOLD);
        backgroundDetail = text("正在读取系统设置", 13, TEXT_MUTED, Typeface.NORMAL);
        backgroundDetail.setPadding(0, dp(5), 0, dp(12));
        backgroundCard.addView(backgroundTitle);
        backgroundCard.addView(backgroundDetail);
        backgroundButton = secondaryButton("完善后台设置");
        backgroundButton.setOnClickListener(v -> configureReliableBackground());
        backgroundCard.addView(backgroundButton, matchHeight(dp(46)));
        root.addView(backgroundCard, spacedCard());

        deviceSectionTitle = addSection(root, "设备", "仅可信设备可以收发剪贴板");
        devicesList = column();
        root.addView(devicesList);
        refreshDevices();

        TextView privacy = text("所有内容仅在局域网内加密传输，不经过云端。", 12, TEXT_MUTED, Typeface.NORMAL);
        privacy.setGravity(Gravity.CENTER);
        privacy.setPadding(dp(8), dp(20), dp(8), 0);
        root.addView(privacy);

        setContentView(scroll);
    }

    private void configureReliableBackground() {
        if (!SpanKeepAliveService.isEnabled(this)) {
            setStatus("需要开启无障碍服务", "在“已下载的应用”中开启 Span 后台同步");
            startActivity(new Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS));
            return;
        }
        PowerManager power = (PowerManager) getSystemService(POWER_SERVICE);
        if (power != null && !power.isIgnoringBatteryOptimizations(getPackageName())) {
            setStatus("需要允许后台耗电", "请选择允许，避免系统休眠同步服务");
            startActivity(new Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS)
                    .setData(Uri.parse("package:" + getPackageName())));
            return;
        }
        SpanReceiveService.start(this);
        setStatus("后台同步已就绪", "无障碍保活与电池设置均已完成");
        updateReliableBackgroundState();
    }

    private void updateReliableBackgroundState() {
        if (backgroundButton == null) return;
        boolean accessibility = SpanKeepAliveService.isEnabled(this);
        PowerManager power = (PowerManager) getSystemService(POWER_SERVICE);
        boolean battery = power != null && power.isIgnoringBatteryOptimizations(getPackageName());
        if (accessibility && battery) {
            backgroundTitle.setText("后台同步已就绪");
            backgroundDetail.setText("可在后台接收；点击系统无障碍按钮可发送");
            backgroundButton.setText("重新检查服务");
            tintButton(backgroundButton, GREEN_SOFT, GREEN, GREEN_SOFT);
        } else if (accessibility) {
            backgroundTitle.setText("还差一步");
            backgroundDetail.setText("请允许 Span 忽略电池优化");
            backgroundButton.setText("允许后台运行");
            tintButton(backgroundButton, ORANGE_SOFT, ORANGE, ORANGE_SOFT);
        } else {
            backgroundTitle.setText("开启可靠后台");
            backgroundDetail.setText("防止小米等系统清理接收服务，并启用快捷发送");
            backgroundButton.setText("去开启");
            tintButton(backgroundButton, BLUE_SOFT, BLUE, BLUE_SOFT);
        }
    }

    private void refreshDevices() {
        if (devicesList == null) return;
        devicesList.removeAllViews();
        List<SpanDevice> devices = store.loadDevices();
        int trustedCount = 0;
        for (SpanDevice device : devices) if (device.trusted) trustedCount++;
        if (deviceSectionTitle != null) deviceSectionTitle.setText("设备  ·  " + trustedCount + " 台可信");

        if (devices.isEmpty()) {
            LinearLayout empty = card(SURFACE);
            TextView emptyTitle = text("还没有发现设备", 16, TEXT, Typeface.BOLD);
            TextView emptyHint = text("在电脑端打开 Span，然后点击“刷新附近设备”。", 13, TEXT_MUTED, Typeface.NORMAL);
            emptyHint.setPadding(0, dp(6), 0, 0);
            empty.addView(emptyTitle);
            empty.addView(emptyHint);
            devicesList.addView(empty, spacedCard());
            return;
        }

        for (SpanDevice device : devices) devicesList.addView(deviceCard(device), spacedCard());
    }

    private View deviceCard(SpanDevice device) {
        LinearLayout card = card(SURFACE);
        LinearLayout top = row();
        LinearLayout copy = column();
        TextView name = text(device.name == null || device.name.isEmpty() ? "未命名设备" : device.name,
                17, TEXT, Typeface.BOLD);
        String platform = platformName(device.platform);
        String endpoint = device.host == null || device.host.isEmpty() ? "等待在线" : device.host;
        TextView meta = text(platform + "  ·  " + endpoint, 13, TEXT_MUTED, Typeface.NORMAL);
        meta.setPadding(0, dp(4), 0, 0);
        copy.addView(name);
        copy.addView(meta);
        top.addView(copy, new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        TextView badge = badge(device.trusted ? "可信" : "新设备", device.trusted);
        top.addView(badge);
        card.addView(top);

        View divider = new View(this);
        divider.setBackgroundColor(DIVIDER);
        LinearLayout.LayoutParams dividerParams = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, dp(1));
        dividerParams.topMargin = dp(14);
        dividerParams.bottomMargin = dp(10);
        card.addView(divider, dividerParams);

        Button trust = device.trusted ? dangerButton("移除设备") : primaryButton("信任并开始同步");
        trust.setOnClickListener(v -> {
            store.setTrusted(device.id, !device.trusted);
            setStatus(device.trusted ? "已移除设备" : "设备已信任",
                    device.name + (device.trusted ? " 不再参与同步" : " 现在可以收发剪贴板"));
            refreshDevices();
        });
        card.addView(trust, matchHeight(dp(44)));
        return card;
    }

    private void sendCurrentClipboard() {
        sendButton.setEnabled(false);
        sendButton.setText("正在发送…");
        worker.execute(() -> {
            try {
                int sent = SpanClipboardSync.sendCurrentClipboard(this);
                runOnUiThread(() -> {
                    sendButton.setEnabled(true);
                    sendButton.setText("发送当前剪贴板");
                    if (sent > 0) {
                        setStatus("发送成功", "已发送到 " + sent + " 台可信设备");
                        Toast.makeText(this, "剪贴板已发送", Toast.LENGTH_SHORT).show();
                    } else {
                        setStatus("没有可发送的内容", "请先复制文本，并确认至少有一台可信设备");
                    }
                });
            } catch (SecurityException error) {
                runOnUiThread(() -> sendFailed("系统暂时不允许读取剪贴板"));
            } catch (Exception error) {
                runOnUiThread(() -> sendFailed("发送失败，请检查设备是否在线"));
            }
        });
    }

    private void sendFailed(String detail) {
        sendButton.setEnabled(true);
        sendButton.setText("重新发送");
        setStatus("未能发送", detail);
        Toast.makeText(this, detail, Toast.LENGTH_SHORT).show();
    }

    private void sendClipboardAfterWake() {
        try {
            int sent = SpanClipboardSync.sendCurrentClipboard(this);
            if (sent > 0) runOnUiThread(() -> setStatus("发送成功", "已发送到 " + sent + " 台可信设备"));
        } catch (SecurityException error) {
            runOnUiThread(() -> setStatus("等待操作", "点击“发送当前剪贴板”重试"));
        } catch (Exception error) {
            runOnUiThread(() -> setStatus("发送失败", "请检查可信设备是否在线"));
        }
    }

    private void requestNotificationPermission() {
        if (android.os.Build.VERSION.SDK_INT >= 33
                && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[]{Manifest.permission.POST_NOTIFICATIONS}, 1001);
        }
    }

    private void handleLaunchIntent(Intent intent) {
        if (intent == null) return;
        if (Intent.ACTION_SEND.equals(intent.getAction()) && "text/plain".equals(intent.getType())) {
            String text = intent.getStringExtra(Intent.EXTRA_TEXT);
            if (text != null && !text.trim().isEmpty()) sendText(text, true);
        }
    }

    private void sendText(String text, boolean finishAfter) {
        worker.execute(() -> {
            try {
                int sent = SpanClipboardSync.sendSharedText(this, text);
                runOnUiThread(() -> {
                    setStatus(sent == 0 ? "没有可信设备" : "发送成功",
                            sent == 0 ? "请先完成设备配对" : "已发送到 " + sent + " 台设备");
                    if (finishAfter) {
                        Toast.makeText(this, sent == 0 ? "没有可信设备" : "已通过 Span 发送", Toast.LENGTH_SHORT).show();
                        finish();
                    }
                });
            } catch (Exception error) {
                runOnUiThread(() -> {
                    setStatus("发送失败", "请检查网络和设备状态");
                    if (finishAfter) Toast.makeText(this, "发送失败", Toast.LENGTH_SHORT).show();
                });
            }
        });
    }

    private void setStatus(String title, String detail) {
        if (statusTitle != null) statusTitle.setText(title);
        if (statusDetail != null) statusDetail.setText(detail);
    }

    private TextView addSection(LinearLayout root, String title, String hint) {
        TextView section = text(title, 18, TEXT, Typeface.BOLD);
        section.setPadding(0, dp(18), 0, 0);
        root.addView(section);
        TextView description = text(hint, 13, TEXT_MUTED, Typeface.NORMAL);
        description.setPadding(0, dp(3), 0, dp(8));
        root.addView(description);
        return section;
    }

    private LinearLayout card(int color) {
        LinearLayout view = column();
        view.setPadding(dp(16), dp(16), dp(16), dp(16));
        view.setBackground(rounded(color, color, 18));
        view.setElevation(dp(1));
        return view;
    }

    private TextView badge(String label, boolean trusted) {
        TextView view = text(label, 12, trusted ? GREEN : ORANGE, Typeface.BOLD);
        view.setGravity(Gravity.CENTER);
        view.setPadding(dp(10), dp(5), dp(10), dp(5));
        view.setBackground(rounded(trusted ? GREEN_SOFT : ORANGE_SOFT,
                trusted ? GREEN_SOFT : ORANGE_SOFT, 20));
        return view;
    }

    private Button primaryButton(String label) {
        Button button = button(label);
        tintButton(button, BLUE, Color.WHITE, BLUE);
        return button;
    }

    private Button secondaryButton(String label) {
        Button button = button(label);
        tintButton(button, BLUE_SOFT, BLUE, BLUE_SOFT);
        return button;
    }

    private Button dangerButton(String label) {
        Button button = button(label);
        tintButton(button, Color.rgb(255, 239, 239), Color.rgb(190, 54, 54), Color.rgb(255, 239, 239));
        return button;
    }

    private Button button(String label) {
        Button button = new Button(this);
        button.setText(label);
        button.setTextSize(15);
        button.setTypeface(Typeface.DEFAULT, Typeface.BOLD);
        button.setAllCaps(false);
        button.setGravity(Gravity.CENTER);
        button.setPadding(dp(12), 0, dp(12), 0);
        button.setStateListAnimator(null);
        return button;
    }

    private void tintButton(Button button, int background, int foreground, int border) {
        button.setTextColor(foreground);
        button.setBackground(rounded(background, border, 14));
    }

    private GradientDrawable rounded(int fill, int stroke, int radiusDp) {
        GradientDrawable drawable = new GradientDrawable();
        drawable.setColor(fill);
        drawable.setCornerRadius(dp(radiusDp));
        drawable.setStroke(dp(1), stroke);
        return drawable;
    }

    private TextView text(String value, float size, int color, int style) {
        TextView view = new TextView(this);
        view.setText(value);
        view.setTextSize(size);
        view.setTextColor(color);
        view.setTypeface(Typeface.DEFAULT, style);
        view.setLineSpacing(0, 1.08f);
        return view;
    }

    private LinearLayout column() {
        LinearLayout view = new LinearLayout(this);
        view.setOrientation(LinearLayout.VERTICAL);
        return view;
    }

    private LinearLayout row() {
        LinearLayout view = new LinearLayout(this);
        view.setOrientation(LinearLayout.HORIZONTAL);
        view.setGravity(Gravity.CENTER_VERTICAL);
        return view;
    }

    private LinearLayout.LayoutParams spacedCard() {
        LinearLayout.LayoutParams params = matchWrap();
        params.bottomMargin = dp(4);
        return params;
    }

    private LinearLayout.LayoutParams matchWrap() {
        return new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
    }

    private LinearLayout.LayoutParams matchHeight(int height) {
        return new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, height);
    }

    private String platformName(String platform) {
        if (platform == null) return "未知平台";
        if ("android".equalsIgnoreCase(platform)) return "Android";
        if ("macos".equalsIgnoreCase(platform)) return "macOS";
        if ("windows".equalsIgnoreCase(platform)) return "Windows";
        if ("linux".equalsIgnoreCase(platform)) return "Linux";
        return platform;
    }

    private int dp(int value) {
        return (int) (value * getResources().getDisplayMetrics().density + 0.5f);
    }
}
