package ma.s2c.maroc;

import android.content.Context;
import android.content.SharedPreferences;
import android.os.Handler;
import android.os.Looper;

import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * The JSON client for this project's own /api/v1, the one the web app serves and
 * docs/API.md documents. Nothing else is spoken to: no third-party SDK, no login page
 * scrape, and the bearer token never leaves the phone except on these requests.
 */
final class Api {

    interface Callback {
        void ok(JSONObject body);

        void fail(String message);
    }

    /** One worker: this is a phone looking at a shop's ledger, not a crawler. */
    private static final ExecutorService POOL = Executors.newSingleThreadExecutor();
    private static final Handler MAIN = new Handler(Looper.getMainLooper());

    private static final String PREFS = "s2c";

    private final Context ctx;
    private final Config cfg;

    Api(Context ctx, Config cfg) {
        this.ctx = ctx.getApplicationContext();
        this.cfg = cfg;
    }

    private SharedPreferences prefs() {
        return ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
    }

    String server() {
        return cfg.server();
    }

    String userName() {
        return prefs().getString("user", "");
    }

    String company() {
        String stored = prefs().getString("company", "");
        return stored.isEmpty() || cfg.isLocal() ? cfg.company() : stored;
    }

    String currency() {
        String stored = prefs().getString("currency", "");
        return stored.isEmpty() || cfg.isLocal() ? cfg.currency() : stored;
    }

    boolean signedIn() {
        return !prefs().getString("token", "").isEmpty();
    }

    void signOut() {
        prefs().edit().remove("token").apply();
    }

    /** Signs in and remembers the token, the company it belongs to and its currency. */
    void login(String server, String username, String password, Callback cb) {
        cfg.setServer(server);
        cfg.save();
        JSONObject body = new JSONObject();
        try {
            body.put("username", username);
            body.put("password", password);
        } catch (Exception ignored) {
            // JSONObject.put only throws for non-finite numbers; two strings cannot.
        }
        send("POST", "/auth/login", body.toString(), new Callback() {
            @Override
            public void ok(JSONObject res) {
                try {
                    JSONObject user = res.optJSONObject("user");
                    JSONObject company = res.optJSONObject("company");
                    prefs().edit()
                            .putString("token", res.optString("token", ""))
                            .putString("user", user == null ? username : user.optString("full_name", username))
                            .putString("company", company == null ? "" : company.optString("name", ""))
                            .putString("currency", company == null ? "MAD" : company.optString("currency", "MAD"))
                            .apply();
                } catch (Exception ignored) {
                    // A token we could not unpack is caught by signedIn() anyway.
                }
                cb.ok(res);
            }

            @Override
            public void fail(String message) {
                prefs().edit().remove("token").apply();
                cb.fail(message);
            }
        });
    }

    void get(String path, Callback cb) {
        send("GET", path, null, cb);
    }

    private void put(String key, String value) {
        prefs().edit().putString(key, value).apply();
    }

    private void send(final String method, final String path, final String body, final Callback cb) {
        POOL.execute(() -> {
            try {
                JSONObject res = request(method, path, body);
                MAIN.post(() -> cb.ok(res));
            } catch (final Exception e) {
                final String message = e.getMessage() == null ? e.toString() : e.getMessage();
                MAIN.post(() -> cb.fail(message));
            }
        });
    }

    private JSONObject request(String method, String path, String body) throws Exception {
        HttpURLConnection c = (HttpURLConnection) new URL(server() + "/api/v1" + path).openConnection();
        try {
            c.setRequestMethod(method);
            c.setConnectTimeout(8000);
            c.setReadTimeout(15000);
            c.setRequestProperty("Accept", "application/json");

            String token = prefs().getString("token", "");
            if (!token.isEmpty()) {
                c.setRequestProperty("Authorization", "Bearer " + token);
            }
            if (body != null) {
                c.setDoOutput(true);
                c.setRequestProperty("Content-Type", "application/json");
                try (OutputStream out = c.getOutputStream()) {
                    out.write(body.getBytes(StandardCharsets.UTF_8));
                }
            }

            int code = c.getResponseCode();
            InputStream in = code >= 400 ? c.getErrorStream() : c.getInputStream();
            String text = in == null ? "" : read(in);

            if (code == 401) {
                throw new Exception("Sign-in refused — check the password, then the server address.");
            }
            if (code >= 400) {
                throw new Exception("The server answered " + code + ". " + shorten(text));
            }
            return new JSONObject(text.isEmpty() ? "{}" : text);
        } finally {
            c.disconnect();
        }
    }

    private static String read(InputStream in) throws Exception {
        try (InputStream src = in; ByteArrayOutputStream out = new ByteArrayOutputStream()) {
            byte[] buf = new byte[8192];
            int n;
            while ((n = src.read(buf)) > 0) {
                out.write(buf, 0, n);
            }
            return out.toString(StandardCharsets.UTF_8.name());
        }
    }

    private static String shorten(String text) {
        String one = text.replaceAll("\\s+", " ").trim();
        return one.length() > 160 ? one.substring(0, 160) + "…" : one;
    }

    private static String trim(String server) {
        String s = server == null ? "" : server.trim();
        while (s.endsWith("/")) {
            s = s.substring(0, s.length() - 1);
        }
        return s.isEmpty() ? Config.DEFAULT_SERVER : s;
    }
}
