package ma.s2c.maroc;

import android.content.Context;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.util.Properties;

/**
 * Settings kept in a plain text file, not in the compiled code, so a shop can change the
 * database it uses, the server address, the company name and the currency later without a
 * new build. The file lives in the app's own folder on the phone:
 *
 *     Android/data/ma.s2c.maroc/files/s2c.properties
 *
 * It is written on the first run (so it is there to be edited by hand over USB, or by any
 * file manager that can see app folders) and it survives updates — only uninstalling the
 * app removes it. The in-app Settings screen edits the same file.
 */
final class Config {

    static final String FILE_NAME = "s2c.properties";

    /** The phone's own SQLite database. The app is complete on its own. */
    static final String MODE_LOCAL = "local";
    /** The office server's database, read over the LAN. */
    static final String MODE_SERVER = "server";

    static final String KEY_MODE = "db.mode";
    static final String KEY_SERVER = "server.url";
    static final String KEY_COMPANY = "company.name";
    static final String KEY_CURRENCY = "company.currency";

    /** The emulator reaches the host machine on 10.0.2.2; a real phone needs the LAN IP. */
    static final String DEFAULT_SERVER = "http://10.0.2.2:8080";

    private final File file;
    private final Properties props = new Properties();

    Config(Context ctx) {
        File dir = ctx.getApplicationContext().getExternalFilesDir(null);
        if (dir == null) {
            // No external storage (rare): fall back to the private folder so the app still runs.
            dir = ctx.getApplicationContext().getFilesDir();
        }
        file = new File(dir, FILE_NAME);
        if (file.isFile()) {
            try (FileInputStream in = new FileInputStream(file)) {
                props.load(in);
            } catch (Exception ignored) {
                // A broken file must not stop the shop: the defaults below still work.
            }
        }
        // Write it out on first run and fill in anything a hand edit removed.
        boolean missing = !file.isFile();
        for (String[] pair : defaults()) {
            if (props.getProperty(pair[0]) == null) {
                props.setProperty(pair[0], pair[1]);
                missing = true;
            }
        }
        if (missing) {
            save();
        }
    }

    private static String[][] defaults() {
        return new String[][]{
                {KEY_MODE, MODE_LOCAL},
                {KEY_SERVER, DEFAULT_SERVER},
                {KEY_COMPANY, "S2C MAROC"},
                {KEY_CURRENCY, "MAD"},
        };
    }

    String path() {
        return file.getAbsolutePath();
    }

    private String get(String key, String fallback) {
        String v = props.getProperty(key, fallback);
        v = v == null ? "" : v.trim();
        return v.isEmpty() ? fallback : v;
    }

    boolean isLocal() {
        return !MODE_SERVER.equalsIgnoreCase(get(KEY_MODE, MODE_LOCAL));
    }

    String mode() {
        return isLocal() ? MODE_LOCAL : MODE_SERVER;
    }

    String server() {
        String s = get(KEY_SERVER, DEFAULT_SERVER);
        while (s.endsWith("/")) {
            s = s.substring(0, s.length() - 1);
        }
        return s;
    }

    String company() {
        return get(KEY_COMPANY, "S2C MAROC");
    }

    String currency() {
        return get(KEY_CURRENCY, "MAD");
    }

    void setMode(String mode) {
        props.setProperty(KEY_MODE, MODE_SERVER.equalsIgnoreCase(mode) ? MODE_SERVER : MODE_LOCAL);
    }

    void setServer(String url) {
        String s = url == null ? "" : url.trim();
        while (s.endsWith("/")) {
            s = s.substring(0, s.length() - 1);
        }
        props.setProperty(KEY_SERVER, s.isEmpty() ? DEFAULT_SERVER : s);
    }

    void setCompany(String name) {
        String s = name == null ? "" : name.trim();
        props.setProperty(KEY_COMPANY, s.isEmpty() ? "S2C MAROC" : s);
    }

    void setCurrency(String currency) {
        String s = currency == null ? "" : currency.trim().toUpperCase(java.util.Locale.US);
        props.setProperty(KEY_CURRENCY, s.isEmpty() ? "MAD" : s);
    }

    /** Writes the file back. Returns null on success, or why it failed. */
    String save() {
        try (FileOutputStream out = new FileOutputStream(file)) {
            props.store(out, "S2C MAROC — phone app settings. Edit with care; the app reads this on start.");
            return null;
        } catch (Exception e) {
            return e.getMessage() == null ? e.toString() : e.getMessage();
        }
    }
}
