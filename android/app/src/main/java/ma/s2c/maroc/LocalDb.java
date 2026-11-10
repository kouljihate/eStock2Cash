package ma.s2c.maroc;

import android.content.ContentValues;
import android.content.Context;
import android.database.Cursor;
import android.database.sqlite.SQLiteDatabase;
import android.database.sqlite.SQLiteOpenHelper;

import org.json.JSONArray;
import org.json.JSONObject;

import java.text.SimpleDateFormat;
import java.util.Date;
import java.util.Locale;
import java.util.UUID;

/**
 * The phone's own database: SQLite, on the device, with no server behind it. This is what
 * makes the app stand alone — suppliers, customers and products live here, and every screen
 * reads them from here when the configured mode is `local`.
 *
 * Rows come back as JSON objects shaped like the ones /api/v1 returns, so the same list
 * renderer draws both a local row and a downloaded one.
 */
final class LocalDb extends SQLiteOpenHelper {

    private static final String NAME = "s2c.db";
    private static final int VERSION = 1;

    static final String SUPPLIERS = "suppliers";
    static final String CUSTOMERS = "customers";
    static final String PRODUCTS = "products";

    LocalDb(Context ctx) {
        super(ctx.getApplicationContext(), NAME, null, VERSION);
    }

    @Override
    public void onCreate(SQLiteDatabase db) {
        db.execSQL("CREATE TABLE suppliers ("
                + "id TEXT PRIMARY KEY, name TEXT NOT NULL, code TEXT, contact TEXT, phone TEXT,"
                + "email TEXT, city TEXT, address TEXT, active INTEGER NOT NULL DEFAULT 1, created_at TEXT)");
        db.execSQL("CREATE TABLE customers ("
                + "id TEXT PRIMARY KEY, name TEXT NOT NULL, code TEXT, contact TEXT, phone TEXT,"
                + "email TEXT, city TEXT, address TEXT, credit_limit REAL NOT NULL DEFAULT 0,"
                + "active INTEGER NOT NULL DEFAULT 1, created_at TEXT)");
        db.execSQL("CREATE TABLE products ("
                + "id TEXT PRIMARY KEY, name TEXT NOT NULL, sku TEXT, category TEXT, unit TEXT,"
                + "cost REAL NOT NULL DEFAULT 0, price REAL NOT NULL DEFAULT 0,"
                + "qty_on_hand REAL NOT NULL DEFAULT 0, reorder_level REAL NOT NULL DEFAULT 0,"
                + "active INTEGER NOT NULL DEFAULT 1, created_at TEXT)");
        db.execSQL("CREATE INDEX idx_suppliers_name ON suppliers(name)");
        db.execSQL("CREATE INDEX idx_customers_name ON customers(name)");
        db.execSQL("CREATE INDEX idx_products_name ON products(name)");
    }

    @Override
    public void onUpgrade(SQLiteDatabase db, int from, int to) {
        // Nothing to migrate yet; the tables are additive from here on.
    }

    /* --------------------------------------------------------------- reading */

    /** The counters the dashboard shows, read from this phone's tables. */
    JSONObject stats() {
        JSONObject out = new JSONObject();
        SQLiteDatabase db = getReadableDatabase();
        try {
            out.put("suppliers", count(db, SUPPLIERS, null, null));
            out.put("customers", count(db, CUSTOMERS, null, null));
            out.put("products", count(db, PRODUCTS, null, null));
            out.put("low_stock", count(db, PRODUCTS, "qty_on_hand <= reorder_level AND reorder_level > 0", null));
            out.put("stock_units", scalar(db, "SELECT IFNULL(SUM(qty_on_hand), 0) FROM products"));
            out.put("stock_value", scalar(db, "SELECT IFNULL(SUM(qty_on_hand * cost), 0) FROM products"));
        } catch (Exception ignored) {
            // A half-written JSON object is still better than a crash on the dashboard.
        }
        return out;
    }

    private int count(SQLiteDatabase db, String table, String where, String[] args) {
        String sql = "SELECT COUNT(*) FROM " + table + (where == null ? "" : " WHERE " + where);
        try (Cursor c = db.rawQuery(sql, args)) {
            return c.moveToFirst() ? c.getInt(0) : 0;
        }
    }

    private double scalar(SQLiteDatabase db, String sql) {
        try (Cursor c = db.rawQuery(sql, null)) {
            return c.moveToFirst() ? c.getDouble(0) : 0;
        }
    }

    /** `kind` is the API path segment: suppliers, customers or products. */
    JSONArray list(String kind, String q) {
        String table = table(kind);
        JSONArray out = new JSONArray();
        if (table == null) {
            return out;
        }
        String order = PRODUCTS.equals(table) ? "name COLLATE NOCASE" : "name COLLATE NOCASE";
        String where = "";
        String[] args = null;
        String needle = q == null ? "" : q.trim();
        if (!needle.isEmpty()) {
            String[] cols = PRODUCTS.equals(table)
                    ? new String[]{"name", "sku", "category"}
                    : new String[]{"name", "code", "phone", "city"};
            StringBuilder sb = new StringBuilder("(");
            for (int i = 0; i < cols.length; i++) {
                if (i > 0) {
                    sb.append(" OR ");
                }
                sb.append(cols[i]).append(" LIKE ?");
            }
            sb.append(")");
            where = " WHERE " + sb;
            args = new String[cols.length];
            for (int i = 0; i < cols.length; i++) {
                args[i] = "%" + needle + "%";
            }
        }
        try (Cursor c = getReadableDatabase().rawQuery(
                "SELECT * FROM " + table + where + " ORDER BY " + order, args)) {
            while (c.moveToNext()) {
                out.put(row(c));
            }
        } catch (Exception ignored) {
            // An unreadable table shows as an empty list rather than a crash.
        }
        return out;
    }

    JSONObject find(String kind, String id) {
        String table = table(kind);
        if (table == null) {
            return new JSONObject();
        }
        try (Cursor c = getReadableDatabase().rawQuery(
                "SELECT * FROM " + table + " WHERE id = ?", new String[]{id})) {
            if (c.moveToFirst()) {
                return row(c);
            }
        } catch (Exception ignored) {
            // Fall through to the empty object.
        }
        return new JSONObject();
    }

    private JSONObject row(Cursor c) {
        JSONObject o = new JSONObject();
        try {
            o.put("id", c.getString(c.getColumnIndexOrThrow("id")));
            o.put("name", c.getString(c.getColumnIndexOrThrow("name")));
            if (has(c, "code")) {
                o.put("code", text(c, "code"));
            }
            if (has(c, "sku")) {
                o.put("sku", text(c, "sku"));
                o.put("category", text(c, "category"));
                o.put("unit", text(c, "unit"));
                o.put("cost", c.getDouble(c.getColumnIndexOrThrow("cost")));
                o.put("price", c.getDouble(c.getColumnIndexOrThrow("price")));
                o.put("qty_on_hand", c.getDouble(c.getColumnIndexOrThrow("qty_on_hand")));
                o.put("reorder_level", c.getDouble(c.getColumnIndexOrThrow("reorder_level")));
            }
            if (has(c, "contact")) {
                o.put("contact", text(c, "contact"));
                o.put("phone", text(c, "phone"));
                o.put("email", text(c, "email"));
                o.put("city", text(c, "city"));
                o.put("address", text(c, "address"));
            }
            if (has(c, "credit_limit")) {
                o.put("credit_limit", c.getDouble(c.getColumnIndexOrThrow("credit_limit")));
            }
            o.put("active", c.getInt(c.getColumnIndexOrThrow("active")) != 0);
            o.put("created_at", text(c, "created_at"));
        } catch (Exception ignored) {
            // Missing columns only ever mean an older table; the keys already set stand.
        }
        return o;
    }

    private boolean has(Cursor c, String column) {
        return c.getColumnIndex(column) >= 0;
    }

    private String text(Cursor c, String column) {
        int i = c.getColumnIndex(column);
        return i < 0 || c.isNull(i) ? "" : c.getString(i);
    }

    /* --------------------------------------------------------------- writing */

    /**
     * Inserts or updates one record. Returns null when saved, or a sentence explaining why
     * it was refused (an empty name, a code already used) — the form shows it as-is.
     */
    String save(String kind, JSONObject row) {
        String table = table(kind);
        if (table == null) {
            return "Unknown record type.";
        }
        String name = row.optString("name", "").trim();
        if (name.isEmpty()) {
            return "A name is required.";
        }
        String code = PRODUCTS.equals(table)
                ? row.optString("sku", "").trim()
                : row.optString("code", "").trim();
        if (!code.isEmpty() && codeTaken(table, code, row.optString("id", ""))) {
            return (PRODUCTS.equals(table) ? "SKU " : "Code ") + code + " is already used.";
        }

        ContentValues v = new ContentValues();
        String id = row.optString("id", "");
        boolean create = id.isEmpty();
        if (create) {
            id = UUID.randomUUID().toString();
            v.put("created_at", now());
        }
        v.put("name", name);
        v.put("active", row.optBoolean("active", true) ? 1 : 0);
        if (PRODUCTS.equals(table)) {
            v.put("sku", code);
            v.put("category", row.optString("category", "").trim());
            String unit = row.optString("unit", "").trim();
            v.put("unit", unit.isEmpty() ? "pc" : unit);
            v.put("cost", number(row, "cost"));
            v.put("price", number(row, "price"));
            v.put("qty_on_hand", number(row, "qty_on_hand"));
            v.put("reorder_level", number(row, "reorder_level"));
        } else {
            v.put("code", code);
            v.put("contact", row.optString("contact", "").trim());
            v.put("phone", row.optString("phone", "").trim());
            v.put("email", row.optString("email", "").trim());
            v.put("city", row.optString("city", "").trim());
            v.put("address", row.optString("address", "").trim());
            if (CUSTOMERS.equals(table)) {
                v.put("credit_limit", number(row, "credit_limit"));
            }
        }

        SQLiteDatabase db = getWritableDatabase();
        if (create) {
            v.put("id", id);
            return db.insert(table, null, v) < 0 ? "Could not save the record." : null;
        }
        return db.update(table, v, "id = ?", new String[]{id}) == 0
                ? "That record is gone; go back and open the list again."
                : null;
    }

    boolean remove(String kind, String id) {
        String table = table(kind);
        if (table == null) {
            return false;
        }
        return getWritableDatabase().delete(table, "id = ?", new String[]{id}) > 0;
    }

    private boolean codeTaken(String table, String code, String exceptId) {
        String column = PRODUCTS.equals(table) ? "sku" : "code";
        try (Cursor c = getReadableDatabase().rawQuery(
                "SELECT COUNT(*) FROM " + table + " WHERE " + column + " = ? AND id <> ?",
                new String[]{code, exceptId == null ? "" : exceptId})) {
            return c.moveToFirst() && c.getInt(0) > 0;
        }
    }

    private double number(JSONObject row, String key) {
        String raw = row.optString(key, "").trim().replace(',', '.');
        if (raw.isEmpty()) {
            return 0;
        }
        try {
            return Double.parseDouble(raw);
        } catch (NumberFormatException e) {
            return 0;
        }
    }

    private String table(String kind) {
        String k = kind == null ? "" : kind;
        int slash = k.lastIndexOf('/');
        if (slash >= 0) {
            k = k.substring(slash + 1);
        }
        if (SUPPLIERS.equals(k) || CUSTOMERS.equals(k) || PRODUCTS.equals(k)) {
            return k;
        }
        return null;
    }

    private String now() {
        return new SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss'Z'", Locale.US).format(new Date());
    }
}
