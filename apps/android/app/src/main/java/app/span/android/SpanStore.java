package app.span.android;

import android.content.Context;
import android.content.SharedPreferences;
import android.os.Build;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

final class SpanStore {
    private static final String PREFS = "span";
    private final SharedPreferences prefs;

    SpanStore(Context context) {
        this.prefs = context.getApplicationContext().getSharedPreferences(PREFS, Context.MODE_PRIVATE);
    }

    LocalIdentity loadOrCreateIdentity() throws Exception {
        String id = prefs.getString("identity.id", null);
        String name = prefs.getString("identity.name", null);
        String privateKey = prefs.getString("identity.private", null);
        String publicKey = prefs.getString("identity.public", null);
        if (id != null && name != null && privateKey != null && publicKey != null) {
            return new LocalIdentity(id, name, privateKey, publicKey);
        }
        LocalIdentity identity = SpanCrypto.createIdentity(Build.MODEL == null ? "Android" : Build.MODEL);
        prefs.edit()
                .putString("identity.id", identity.id)
                .putString("identity.name", identity.name)
                .putString("identity.private", identity.privateKeyHex)
                .putString("identity.public", identity.publicKeyHex)
                .commit();
        return identity;
    }


    boolean isReceiverEnabled() {
        return prefs.getBoolean("receiver.enabled", true);
    }

    void setReceiverEnabled(boolean enabled) {
        prefs.edit().putBoolean("receiver.enabled", enabled).commit();
    }

    SpanDevice trustedDevice(String id) {
        for (SpanDevice device : loadDevices()) {
            if (device.trusted && device.id.equals(id)) return device;
        }
        return null;
    }

    List<SpanDevice> loadDevices() {
        List<SpanDevice> devices = new ArrayList<>();
        String raw = prefs.getString("devices", "[]");
        try {
            JSONArray array = new JSONArray(raw);
            for (int i = 0; i < array.length(); i++) {
                JSONObject o = array.getJSONObject(i);
                devices.add(new SpanDevice(
                        o.optString("id"),
                        o.optString("name"),
                        o.optString("platform"),
                        o.optString("host", null),
                        o.optString("publicKeyHex", null),
                        o.optBoolean("trusted"),
                        o.optLong("lastSeenMillis")
                ));
            }
        } catch (Exception ignored) {}
        return devices;
    }

    void saveDevices(List<SpanDevice> devices) {
        devices = compactDevices(devices);
        JSONArray array = new JSONArray();
        try {
            for (SpanDevice d : devices) {
                JSONObject o = new JSONObject();
                o.put("id", d.id);
                o.put("name", d.name);
                o.put("platform", d.platform);
                o.put("host", d.host);
                o.put("publicKeyHex", d.publicKeyHex);
                o.put("trusted", d.trusted);
                o.put("lastSeenMillis", d.lastSeenMillis);
                array.put(o);
            }
        } catch (Exception ignored) {}
        // Trust is security state. Persist it before returning so a process kill
        // or reboot immediately after pairing cannot discard the confirmation.
        prefs.edit().putString("devices", array.toString()).commit();
    }

    void upsertDiscovered(SpanDevice discovered) {
        List<SpanDevice> devices = loadDevices();
        for (SpanDevice existing : devices) {
            if (sameDevice(existing, discovered)) {
                boolean trusted = existing.trusted || discovered.trusted;
                boolean keyMatches = existing.publicKeyHex == null
                        || discovered.publicKeyHex == null
                        || existing.publicKeyHex.equalsIgnoreCase(discovered.publicKeyHex);
                existing.name = discovered.name;
                existing.platform = discovered.platform;
                // A trusted device key is pinned. An unauthenticated discovery
                // packet must not silently redirect trusted traffic.
                if (!existing.trusted || keyMatches) {
                    existing.host = discovered.host;
                    existing.publicKeyHex = discovered.publicKeyHex;
                }
                existing.lastSeenMillis = discovered.lastSeenMillis;
                existing.trusted = trusted;
                saveDevices(devices);
                return;
            }
        }
        devices.add(discovered);
        saveDevices(devices);
    }

    private List<SpanDevice> compactDevices(List<SpanDevice> devices) {
        List<SpanDevice> compacted = new ArrayList<>();
        for (SpanDevice device : devices) {
            SpanDevice existing = null;
            for (SpanDevice candidate : compacted) {
                if (sameDevice(candidate, device)) {
                    existing = candidate;
                    break;
                }
            }
            if (existing == null) {
                compacted.add(device);
                continue;
            }
            boolean keyConflict = existing.publicKeyHex != null
                    && device.publicKeyHex != null
                    && !existing.publicKeyHex.equalsIgnoreCase(device.publicKeyHex);
            if (keyConflict && existing.trusted) continue;
            existing.trusted = existing.trusted || device.trusted;
            existing.id = device.id;
            existing.name = device.name;
            existing.platform = device.platform;
            existing.host = device.host == null ? existing.host : device.host;
            if (!existing.trusted || existing.publicKeyHex == null) existing.publicKeyHex = device.publicKeyHex;
            existing.lastSeenMillis = Math.max(existing.lastSeenMillis, device.lastSeenMillis);
        }
        return compacted;
    }

    private boolean sameDevice(SpanDevice left, SpanDevice right) {
        if (left.id != null && left.id.equals(right.id)) return true;
        if (left.publicKeyHex != null && right.publicKeyHex != null
                && left.publicKeyHex.equalsIgnoreCase(right.publicKeyHex)) return true;
        return left.name != null && left.name.equals(right.name)
                && left.platform != null && left.platform.equals(right.platform);
    }

    void setTrusted(String id, boolean trusted) {
        List<SpanDevice> devices = loadDevices();
        for (SpanDevice device : devices) {
            if (device.id.equals(id)) device.trusted = trusted;
        }
        saveDevices(devices);
    }
}
