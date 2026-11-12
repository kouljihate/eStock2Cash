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
    private static final int VERSION = 2;

    static final String SUPPLIERS = "suppliers";
    static final String CUSTOMERS = "customers";
    static final String PRODUCTS = "products";
    static final String SALES = "sales";

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
        createSales(db);
    }

    /**
     * The till: one row per sale and one per line, plus what was owed when the sale went out
     * on credit. Kept apart from the master data because a sale is history — deleting a
     * product must never delete what has already been sold.
     */
    private void createSales(SQLiteDatabase db) {
        db.execSQL("CREATE TABLE IF NOT EXISTS sales ("
                + "id TEXT PRIMARY KEY, number TEXT, date TEXT, customer_id TEXT, customer_name TEXT,"
                + "payment TEXT NOT NULL DEFAULT 'cash', items INTEGER NOT NULL DEFAULT 0,"
                + "total REAL NOT NULL DEFAULT 0, paid REAL NOT NULL DEFAULT 0,"
                + "status TEXT NOT NULL DEFAULT 'paid', created_at TEXT)");
        db.execSQL("CREATE TABLE IF NOT EXISTS sale_items ("
                + "id TEXT PRIMARY KEY, sale_id TEXT NOT NULL, product_id TEXT, name TEXT,"
                + "qty REAL NOT NULL DEFAULT 0, price REAL NOT NULL DEFAULT 0, cost REAL NOT NULL DEFAULT 0)");
        db.execSQL("CREATE INDEX IF NOT EXISTS idx_sales_customer ON sales(customer_id)");
        db.execSQL("CREATE INDEX IF NOT EXISTS idx_sale_items_sale ON sale_items(sale_id)");
    }

    @Override
    public void onUpgrade(SQLiteDatabase db, int from, int to) {
        // Tables are only ever added: an installed phone keeps its data across an upgrade.
        if (from < 2) {
            createSales(db);
        }
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
        return scalar(db, sql, null);
    }

    private double scalar(SQLiteDatabase db, String sql, String[] args) {
        try (Cursor c = db.rawQuery(sql, args)) {
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

    /* ----------------------------------------------------------------- the till */

    /** The next sale number, e.g. S-0007, counted from the sales already in the phone. */
    String nextNumber() {
        int done = count(getReadableDatabase(), SALES, null, null);
        return String.format(Locale.US, "S-%04d", done + 1);
    }

    /**
     * Records one till sale and takes its quantities out of stock.
     *
     * @param sale  number, payment (cash or credit), customer_id, customer_name, total
     * @param items one object per line: product_id, name, qty, price, cost
     * @return null when saved, or the sentence the cashier should read instead
     */
    String saveSale(JSONObject sale, JSONArray items) {
        if (items == null || items.length() == 0) {
            return "The cart is empty.";
        }
        SQLiteDatabase db = getWritableDatabase();
        String saleId = UUID.randomUUID().toString();
        double total = 0;
        db.beginTransaction();
        try {
            for (int i = 0; i < items.length(); i++) {
                JSONObject line = items.getJSONObject(i);
                String productId = line.optString("product_id", "");
                double qty = line.optDouble("qty", 0);
                if (qty <= 0) {
                    return "One line has no quantity.";
                }
                double have = scalar(db, "SELECT qty_on_hand FROM products WHERE id = ?", new String[]{productId});
                if (qty > have) {
                    return "Only " + qtyText(have) + " of " + line.optString("name", "that item") + " left in stock.";
                }
                ContentValues v = new ContentValues();
                v.put("id", UUID.randomUUID().toString());
                v.put("sale_id", saleId);
                v.put("product_id", productId);
                v.put("name", line.optString("name", ""));
                v.put("qty", qty);
                v.put("price", line.optDouble("price", 0));
                v.put("cost", line.optDouble("cost", 0));
                if (db.insert("sale_items", null, v) < 0) {
                    return "Could not save the sale.";
                }
                db.execSQL("UPDATE products SET qty_on_hand = qty_on_hand - ? WHERE id = ?",
                        new Object[]{qty, productId});
                total += qty * line.optDouble("price", 0);
            }

            boolean credit = "credit".equals(sale.optString("payment", "cash"));
            ContentValues s = new ContentValues();
            s.put("id", saleId);
            s.put("number", sale.optString("number", nextNumber()));
            s.put("date", now());
            s.put("customer_id", sale.optString("customer_id", ""));
            s.put("customer_name", sale.optString("customer_name", ""));
            s.put("payment", credit ? "credit" : "cash");
            s.put("items", items.length());
            s.put("total", total);
            s.put("paid", credit ? 0 : total);
            s.put("status", credit ? "credit" : "paid");
            s.put("created_at", now());
            if (db.insert(SALES, null, s) < 0) {
                return "Could not save the sale.";
            }
            db.setTransactionSuccessful();
            return null;
        } catch (Exception e) {
            return "Could not save the sale: " + e.getMessage();
        } finally {
            db.endTransaction();
        }
    }

    /** The last sales, newest first, for the cashier's own history. */
    JSONArray sales(int limit) {
        JSONArray out = new JSONArray();
        try (Cursor c = getReadableDatabase().rawQuery(
                "SELECT * FROM sales ORDER BY created_at DESC LIMIT " + Math.max(1, limit), null)) {
            while (c.moveToNext()) {
                JSONObject o = new JSONObject();
                o.put("id", text(c, "id"));
                o.put("number", text(c, "number"));
                o.put("date", text(c, "date"));
                o.put("customer_id", text(c, "customer_id"));
                o.put("customer_name", text(c, "customer_name"));
                o.put("payment", text(c, "payment"));
                o.put("items", c.getInt(c.getColumnIndexOrThrow("items")));
                o.put("total", c.getDouble(c.getColumnIndexOrThrow("total")));
                o.put("paid", c.getDouble(c.getColumnIndexOrThrow("paid")));
                o.put("status", text(c, "status"));
                out.put(o);
            }
        } catch (Exception ignored) {
            // An unreadable till shows as an empty history, not a crash.
        }
        return out;
    }

    /**
     * Who owes the shop what: one row per customer with an unpaid sale, plus the total.
     * The Credit tab reads this and nothing else — the phone does not guess a limit.
     */
    JSONObject credit() {
        JSONObject out = new JSONObject();
        JSONArray rows = new JSONArray();
        double sum = 0;
        String sql = "SELECT s.customer_id AS cid, s.customer_name AS name,"
                + " SUM(s.total - s.paid) AS owed, COUNT(*) AS open,"
                + " IFNULL((SELECT c.phone FROM customers c WHERE c.id = s.customer_id), '') AS phone,"
                + " IFNULL((SELECT c.credit_limit FROM customers c WHERE c.id = s.customer_id), 0) AS lim"
                + " FROM sales s WHERE s.paid < s.total GROUP BY s.customer_id ORDER BY owed DESC";
        try (Cursor c = getReadableDatabase().rawQuery(sql, null)) {
            while (c.moveToNext()) {
                JSONObject o = new JSONObject();
                o.put("id", text(c, "cid"));
                o.put("name", text(c, "name").isEmpty() ? "Walk-in" : text(c, "name"));
                o.put("phone", text(c, "phone"));
                o.put("outstanding", c.getDouble(c.getColumnIndexOrThrow("owed")));
                o.put("open_sales", c.getInt(c.getColumnIndexOrThrow("open")));
                o.put("credit_limit", c.getDouble(c.getColumnIndexOrThrow("lim")));
                rows.put(o);
                sum += c.getDouble(c.getColumnIndexOrThrow("owed"));
            }
        } catch (Exception ignored) {
            // Same rule as everywhere else: an empty list beats an exception.
        }
        try {
            out.put("items", rows);
            out.put("total", sum);
            out.put("count", rows.length());
        } catch (Exception ignored) {
            // JSONObject string keys cannot fail.
        }
        return out;
    }

    /** Marks every unpaid sale of one customer as settled — the debt is paid in at the till. */
    boolean settle(String customerId) {
        if (customerId == null || customerId.isEmpty()) {
            return false;
        }
        SQLiteDatabase db = getWritableDatabase();
        // paid is copied from total, so the balance really is zero afterwards.
        db.execSQL("UPDATE sales SET status = 'paid', paid = total WHERE customer_id = ? AND paid < total",
                new Object[]{customerId});
        return true;
    }

    private String qtyText(double v) {
        return v == Math.rint(v) ? String.format(Locale.US, "%,.0f", v) : String.format(Locale.US, "%,.2f", v);
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
