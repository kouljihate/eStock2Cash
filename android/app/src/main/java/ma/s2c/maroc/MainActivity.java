package ma.s2c.maroc;

import android.app.Activity;
import android.content.Context;
import android.graphics.Typeface;
import android.os.Bundle;
import android.text.InputType;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.inputmethod.EditorInfo;
import android.widget.BaseAdapter;
import android.widget.Button;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.ListView;
import android.widget.ProgressBar;
import android.widget.ScrollView;
import android.widget.TextView;

import org.json.JSONArray;
import org.json.JSONObject;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;

/**
 * The phone app: sign in, a dashboard, and the ledger itself, read straight from the
 * JSON API this project already serves. Screens are built in code rather than in XML
 * because every label here is bilingual and stacked — Arabic above the Latin — which is
 * easier to keep honest in one place than in a pile of layouts.
 */
public class MainActivity extends Activity {

    /** One screen: builds itself on demand, so going back refetches rather than reuses stale rows. */
    private interface Screen {
        View build();
    }

    private Api api;
    private FrameLayout host;
    private final ArrayDeque<Screen> stack = new ArrayDeque<>();

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        api = new Api(this);

        host = new FrameLayout(this);
        host.setBackgroundColor(Ui.BG);
        host.setLayoutDirection(View.LAYOUT_DIRECTION_LTR);
        setContentView(host);

        // Android draws edge-to-edge from API 35 on, which put the navy header under the
        // clock. One inset on the root keeps every screen clear of both bars.
        host.setOnApplyWindowInsetsListener((v, insets) -> {
            v.setPadding(insets.getSystemWindowInsetLeft(), insets.getSystemWindowInsetTop(),
                    insets.getSystemWindowInsetRight(), insets.getSystemWindowInsetBottom());
            return insets;
        });

        push(api.signedIn() ? this::homeScreen : this::loginScreen);
    }

    @Override
    public void onBackPressed() {
        if (stack.size() > 1) {
            stack.pop();
            render();
        } else {
            // The last screen is the dashboard; back leaves the app rather than the account.
            super.onBackPressed();
        }
    }

    private void push(Screen screen) {
        stack.push(screen);
        render();
    }

    private void render() {
        Screen top = stack.peek();
        if (top == null) {
            return;
        }
        host.removeAllViews();
        host.addView(top.build());
    }

    /* ------------------------------------------------------------------ sign in */

    private View loginScreen() {
        LinearLayout page = Ui.column(this);

        LinearLayout head = Ui.column(this);
        head.setBackgroundColor(Ui.DARK);
        int p = Ui.dp(this, 24);
        head.setPadding(p, Ui.dp(this, 40), p, Ui.dp(this, 28));
        head.addView(Ui.text(this, "S2C MAROC", 24, Ui.WHITE, true));
        head.addView(Ui.text(this, "Procure-to-Pay  ·  دورة الشراء", 12, 0xFF94A3B8, false));
        head.addView(Ui.text(this, "Quote-to-Cash  ·  دورة البيع", 12, 0xFF94A3B8, false));
        page.addView(head);

        ScrollView scroll = new ScrollView(this);
        LinearLayout body = Ui.column(this);
        int b = Ui.dp(this, 18);
        body.setPadding(b, b, b, b);

        LinearLayout card = Ui.card(this);
        card.addView(Ui.bi(this, "Sign in", "تسجيل الدخول", 18, Ui.INK, true));
        card.addView(Ui.spacer(this, 14));

        final EditText server = field("Server · الخادم", api.server(), InputType.TYPE_TEXT_VARIATION_URI);
        card.addView(fieldLabel("Server address", "عنوان الخادم"));
        card.addView(server);
        card.addView(Ui.spacer(this, 10));

        final EditText user = field("Username · اسم المستخدم", api.userName(), InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD);
        user.setSingleLine(true);
        card.addView(fieldLabel("Username", "اسم المستخدم"));
        card.addView(user);
        card.addView(Ui.spacer(this, 10));

        final EditText pass = field("Password · كلمة المرور", "", InputType.TYPE_TEXT_VARIATION_PASSWORD);
        pass.setSingleLine(true);
        pass.setImeOptions(EditorInfo.IME_ACTION_GO);
        card.addView(fieldLabel("Password", "كلمة المرور"));
        card.addView(pass);

        card.addView(Ui.spacer(this, 16));

        final TextView error = Ui.text(this, "", 12.5f, Ui.RED, false);
        error.setVisibility(View.GONE);
        card.addView(error);

        final Button go = new Button(this);
        go.setAllCaps(false);
        go.setText("Sign in  ·  تسجيل الدخول");
        go.setTextSize(14);
        go.setTextColor(Ui.WHITE);
        go.setTypeface(Typeface.DEFAULT_BOLD);
        go.setBackground(Ui.rounded(Ui.BRAND, 12, this));
        go.setPadding(0, Ui.dp(this, 14), 0, Ui.dp(this, 14));
        card.addView(go, Ui.lp(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));

        final ProgressBar busy = new ProgressBar(this);
        busy.setVisibility(View.GONE);
        card.addView(busy);

        go.setOnClickListener(v -> {
            String s = server.getText().toString().trim();
            String u = user.getText().toString().trim();
            String pw = pass.getText().toString();
            if (u.isEmpty() || pw.isEmpty()) {
                showError(error, "Enter the username and the password.");
                return;
            }
            go.setEnabled(false);
            busy.setVisibility(View.VISIBLE);
            error.setVisibility(View.GONE);
            api.login(s, u, pw, new Api.Callback() {
                @Override
                public void ok(JSONObject body) {
                    go.setEnabled(true);
                    busy.setVisibility(View.GONE);
                    stack.clear();
                    push(MainActivity.this::homeScreen);
                }

                @Override
                public void fail(String message) {
                    go.setEnabled(true);
                    busy.setVisibility(View.GONE);
                    showError(error, message);
                }
            });
        });

        body.addView(card);

        TextView hint = Ui.text(this, "The account is the office server's, not this phone's. The address is "
                + "the PC that runs the app, e.g. http://192.168.1.20:8080", 12, Ui.MUTED, false);
        hint.setPadding(0, Ui.dp(this, 12), 0, 0);
        body.addView(hint);

        scroll.addView(body);
        page.addView(scroll, Ui.lp(ViewGroup.LayoutParams.MATCH_PARENT, 0));
        ((LinearLayout.LayoutParams) scroll.getLayoutParams()).weight = 1;

        page.addView(footer());
        return page;
    }

    private TextView fieldLabel(String latin, String arabic) {
        TextView t = Ui.text(this, latin + "  ·  " + arabic, 12, Ui.MUTED, false);
        t.setPadding(0, Ui.dp(this, 4), 0, Ui.dp(this, 5));
        return t;
    }

    private EditText field(String hint, String value, int variation) {
        EditText e = new EditText(this);
        e.setHint(hint);
        e.setText(value);
        e.setTextSize(14);
        e.setTextColor(Ui.INK);
        e.setHintTextColor(0xFF94A3B8);
        e.setInputType(InputType.TYPE_CLASS_TEXT | variation);
        int pad = Ui.dp(this, 12);
        e.setPadding(pad, pad, pad, pad);
        e.setBackground(Ui.outlined(Ui.WHITE, 11, Ui.LINE, this));
        e.setLayoutParams(Ui.lp(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        return e;
    }

    /* ---------------------------------------------------------------- dashboard */

    private View homeScreen() {
        LinearLayout page = Ui.column(this);
        page.addView(topBar("Dashboard", "لوحة التحكم", false));

        ScrollView scroll = new ScrollView(this);
        LinearLayout body = Ui.column(this);
        int b = Ui.dp(this, 14);
        body.setPadding(b, b, b, b);

        final LinearLayout stats = Ui.column(this);
        body.addView(stats);

        final LinearLayout trouble = Ui.column(this);
        body.addView(trouble);

        stats.addView(loadingCard("Reading the dashboard…", "جارٍ قراءة لوحة التحكم…"));

        api.get("/dashboard", new Api.Callback() {
            @Override
            public void ok(JSONObject body0) {
                stats.removeAllViews();
                stats.addView(statGrid(body0));
                stats.addView(Ui.spacer(MainActivity.this, 14));
                stats.addView(agingCard(body0));
                stats.addView(Ui.spacer(MainActivity.this, 14));
                stats.addView(sectionCard("Procure-to-Pay", "دورة الشراء", new String[][]{
                        {"Suppliers", "الموردون", "/suppliers", "party"},
                        {"Purchase orders", "أوامر الشراء", "/purchase-orders", "doc"},
                        {"Supplier invoices", "فواتير الموردين", "/supplier-invoices", "doc"},
                        {"Payments", "المدفوعات", "/payments", "payment"},
                }));
                stats.addView(Ui.spacer(MainActivity.this, 12));
                stats.addView(sectionCard("Quote-to-Cash", "دورة البيع", new String[][]{
                        {"Customers", "العملاء", "/customers", "party"},
                        {"Quotes", "عروض الأسعار", "/quotes", "doc"},
                        {"Customer orders", "طلبات العملاء", "/customer-orders", "doc"},
                        {"Customer invoices", "فواتير العملاء", "/customer-invoices", "doc"},
                }));
                stats.addView(Ui.spacer(MainActivity.this, 12));
                stats.addView(sectionCard("In stock", "المخزون", new String[][]{
                        {"Products", "المنتجات", "/products", "product"},
                }));
            }

            @Override
            public void fail(String message) {
                stats.removeAllViews();
                stats.addView(problemCard(message, "Could not read the dashboard.", "تعذّر قراءة لوحة التحكم.", () -> {
                    stack.pop();
                    push(MainActivity.this::homeScreen);
                }));
            }
        });

        scroll.addView(body);
        page.addView(scroll, Ui.lp(ViewGroup.LayoutParams.MATCH_PARENT, 0));
        ((LinearLayout.LayoutParams) scroll.getLayoutParams()).weight = 1;

        page.addView(footer());
        return page;
    }

    private View statGrid(JSONObject d) {
        LinearLayout grid = Ui.column(this);
        String cur = api.currency();
        String[][] cells = {
                {"Suppliers", "الموردون", String.valueOf(d.optInt("suppliers"))},
                {"Customers", "العملاء", String.valueOf(d.optInt("customers"))},
                {"Products", "المنتجات", String.valueOf(d.optInt("products"))},
                {"Low stock", "مخزون منخفض", String.valueOf(d.optInt("low_stock"))},
                {"Stock value", "قيمة المخزون", Ui.money(d.optDouble("stock_value", 0), cur)},
                {"Receivables", "المستحق لنا", Ui.money(d.optDouble("receivables", 0), cur)},
                {"Payables", "المستحق علينا", Ui.money(d.optDouble("payables", 0), cur)},
                {"Open POs", "أوامر مفتوحة", String.valueOf(d.optInt("open_purchase_orders"))},
        };
        for (int i = 0; i < cells.length; i += 2) {
            LinearLayout row = Ui.row(this);
            row.addView(statCell(cells[i][1], cells[i][0], cells[i][2]), weighted());
            if (i + 1 < cells.length) {
                row.addView(statCell(cells[i + 1][1], cells[i + 1][0], cells[i + 1][2]), weightedWithGap());
            } else {
                row.addView(new View(this), weightedWithGap());
            }
            grid.addView(row);
        }
        return grid;
    }

    private LinearLayout.LayoutParams weighted() {
        LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f);
        p.rightMargin = Ui.dp(this, 5);
        return p;
    }

    private LinearLayout.LayoutParams weightedWithGap() {
        LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f);
        p.leftMargin = Ui.dp(this, 5);
        p.bottomMargin = Ui.dp(this, 10);
        return p;
    }

    private View statCell(String arabic, String latin, String value) {
        LinearLayout card = Ui.card(this);
        card.setPadding(Ui.dp(this, 12), Ui.dp(this, 12), Ui.dp(this, 12), Ui.dp(this, 12));
        TextView ar = Ui.text(this, arabic, 11, Ui.MUTED, false);
        ar.setGravity(Gravity.LEFT);
        card.addView(ar);
        card.addView(Ui.text(this, latin, 11.5f, Ui.MUTED, false));
        card.addView(Ui.spacer(this, 6));
        card.addView(Ui.text(this, value, 17, Ui.INK, true));
        LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        p.bottomMargin = Ui.dp(this, 10);
        card.setLayoutParams(p);
        return card;
    }

    private View agingCard(JSONObject d) {
        LinearLayout card = Ui.card(this);
        JSONObject aging = d.optJSONObject("aging");
        card.addView(Ui.bi(this, "Receivable aging", "تقادم المستحقات", 14, Ui.INK, true));
        card.addView(Ui.spacer(this, 8));
        if (aging != null) {
            addKv(card, "Current", "حالياً", Ui.money(aging.optDouble("current", 0), api.currency()));
            addKv(card, "1-30 days", "1-30 يوماً", Ui.money(aging.optDouble("d30", 0), api.currency()));
            addKv(card, "31-60 days", "31-60 يوماً", Ui.money(aging.optDouble("d60", 0), api.currency()));
            addKv(card, "60+ days", "أكثر من 60", Ui.money(aging.optDouble("d90", 0), api.currency()));
            card.addView(Ui.hairline(this));
            addKv(card, "Total", "المجموع", Ui.money(aging.optDouble("total", 0), api.currency()));
        }
        return card;
    }

    private void addKv(LinearLayout card, String latin, String arabic, String value) {
        LinearLayout row = Ui.row(this);
        row.addView(Ui.biLine(this, latin, arabic, 12.5f, Ui.MUTED, false), weight(1f));
        row.addView(Ui.text(this, value, 12.5f, Ui.INK, true));
        row.setPadding(0, Ui.dp(this, 3), 0, Ui.dp(this, 3));
        card.addView(row);
    }

    private LinearLayout.LayoutParams weight(float w) {
        return new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, w);
    }

    private View sectionCard(String latin, String arabic, String[][] entries) {
        LinearLayout card = Ui.card(this);
        card.setPadding(Ui.dp(this, 16), Ui.dp(this, 14), Ui.dp(this, 16), Ui.dp(this, 6));
        card.addView(Ui.bi(this, latin, arabic, 13, Ui.BRAND, true));
        card.addView(Ui.spacer(this, 6));
        for (String[] e : entries) {
            card.addView(navRow(e[0], e[1], e[2], e[3]));
        }
        return card;
    }

    private View navRow(String latin, String arabic, String path, String kind) {
        LinearLayout row = Ui.row(this);
        row.setBackgroundColor(0x00000000);
        row.setPadding(0, Ui.dp(this, 10), 0, Ui.dp(this, 10));
        row.addView(Ui.bi(this, latin, arabic, 14, Ui.INK, false), weight(1f));
        row.addView(Ui.text(this, "›", 18, Ui.MUTED, false));
        row.setOnClickListener(v -> push(() -> listScreen(latin, arabic, path, kind)));
        return row;
    }

    /* --------------------------------------------------------------------- list */

    private View listScreen(String latin, String arabic, String path, String kind) {
        LinearLayout page = Ui.column(this);
        page.addView(topBar(latin, arabic, true));

        final ProgressBar busy = new ProgressBar(this);
        busy.setPadding(0, Ui.dp(this, 24), 0, 0);

        final LinearLayout holder = Ui.column(this);
        holder.addView(busy);

        api.get(path + "?limit=500", new Api.Callback() {
            @Override
            public void ok(JSONObject body) {
                holder.removeAllViews();
                JSONArray items = body.optJSONArray("items");
                if (items == null || items.length() == 0) {
                    holder.addView(emptyCard());
                    return;
                }
                ListView list = new ListView(MainActivity.this);
                list.setDivider(null);
                list.setDividerHeight(0);
                list.setPadding(Ui.dp(MainActivity.this, 12), Ui.dp(MainActivity.this, 6),
                        Ui.dp(MainActivity.this, 12), Ui.dp(MainActivity.this, 6));
                list.setClipToPadding(false);
                list.setAdapter(new Rows(MainActivity.this, items, kind));
                holder.addView(list, Ui.lp(ViewGroup.LayoutParams.MATCH_PARENT, 0));
                ((LinearLayout.LayoutParams) list.getLayoutParams()).weight = 1;
            }

            @Override
            public void fail(String message) {
                holder.removeAllViews();
                holder.addView(problemCard(message, "Could not read the list.", "تعذّرت قراءة القائمة.", () -> {
                    stack.pop();
                    push(() -> listScreen(latin, arabic, path, kind));
                }));
            }
        });

        page.addView(holder, Ui.lp(ViewGroup.LayoutParams.MATCH_PARENT, 0));
        ((LinearLayout.LayoutParams) holder.getLayoutParams()).weight = 1;
        page.addView(footer());
        return page;
    }

    private View emptyCard() {
        LinearLayout card = Ui.card(this);
        card.addView(Ui.bi(this, "No records yet.", "لا توجد سجلات بعد.", 14, Ui.MUTED, false));
        LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        p.setMargins(Ui.dp(this, 12), Ui.dp(this, 12), Ui.dp(this, 12), 0);
        card.setLayoutParams(p);
        return card;
    }

    private View loadingCard(String latin, String arabic) {
        LinearLayout card = Ui.card(this);
        card.addView(Ui.bi(this, latin, arabic, 13, Ui.MUTED, false));
        LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        p.bottomMargin = Ui.dp(this, 12);
        card.setLayoutParams(p);
        return card;
    }

    private View problemCard(String message, String latin, String arabic, Runnable retry) {
        LinearLayout card = Ui.card(this);
        card.setBackground(Ui.rounded(0xFFFDEEEE, 16, this));
        card.addView(Ui.bi(this, latin, arabic, 14, Ui.RED, true));
        card.addView(Ui.spacer(this, 6));
        card.addView(Ui.text(this, message, 12.5f, Ui.INK, false));
        card.addView(Ui.spacer(this, 10));
        Button again = new Button(this);
        again.setAllCaps(false);
        again.setText("Try again  ·  إعادة المحاولة");
        again.setTextSize(13);
        again.setTextColor(Ui.WHITE);
        again.setBackground(Ui.rounded(Ui.BRAND, 10, this));
        again.setOnClickListener(v -> retry.run());
        card.addView(again);
        return card;
    }

    private void showError(TextView target, String message) {
        target.setText(message);
        target.setVisibility(View.VISIBLE);
    }

    /* ---------------------------------------------------------------- chrome */

    private View topBar(String latin, String arabic, boolean back) {
        LinearLayout bar = Ui.row(this);
        bar.setBackgroundColor(Ui.DARK);
        int p = Ui.dp(this, 14);
        bar.setPadding(p, Ui.dp(this, 16), p, Ui.dp(this, 14));

        if (back) {
            TextView arrow = Ui.text(this, "‹", 26, Ui.WHITE, false);
            arrow.setPadding(0, 0, Ui.dp(this, 12), 0);
            arrow.setOnClickListener(v -> {
                if (stack.size() > 1) {
                    stack.pop();
                    render();
                }
            });
            bar.addView(arrow);
        }

        LinearLayout titles = Ui.bi(this, latin, arabic, 17, Ui.WHITE, true);
        bar.addView(titles, weight(1f));

        String who = api.company().isEmpty() ? api.userName() : api.company();
        LinearLayout right = Ui.column(this);
        right.setGravity(Gravity.END);
        if (!api.userName().isEmpty()) {
            right.addView(Ui.text(this, who, 11, 0xFF94A3B8, false));
        }
        TextView out = Ui.text(this, back ? "Sign out" : "Sign out  ·  خروج", 12, 0xFFC7D2FE, false);
        out.setOnClickListener(v -> {
            api.signOut();
            stack.clear();
            push(this::loginScreen);
        });
        right.addView(out);
        bar.addView(right);
        return bar;
    }

    private View footer() {
        TextView t = Ui.text(this, "S2C MAROC  ·  S2C المغرب  ·  v0.1.0", 10.5f, Ui.MUTED, false);
        t.setGravity(Gravity.CENTER);
        t.setPadding(0, Ui.dp(this, 8), 0, Ui.dp(this, 10));
        t.setBackgroundColor(0xFFE9EDF6);
        return t;
    }

    /* ---------------------------------------------------------------- rows */

    /** Turns an API record into the two lines a shop floor actually reads. */
    private final class Rows extends BaseAdapter {
        private final Context ctx;
        private final JSONArray items;
        private final String kind;

        Rows(Context ctx, JSONArray items, String kind) {
            this.ctx = ctx;
            this.items = items;
            this.kind = kind;
        }

        @Override
        public int getCount() {
            return items.length();
        }

        @Override
        public Object getItem(int i) {
            return items.optJSONObject(i);
        }

        @Override
        public long getItemId(int i) {
            return i;
        }

        @Override
        public View getView(int i, View reuse, ViewGroup parent) {
            JSONObject o = items.optJSONObject(i);
            LinearLayout card = Ui.card(ctx);
            card.setPadding(Ui.dp(ctx, 14), Ui.dp(ctx, 12), Ui.dp(ctx, 14), Ui.dp(ctx, 12));
            LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT);
            p.bottomMargin = Ui.dp(ctx, 8);
            card.setLayoutParams(p);

            LinearLayout top = Ui.row(ctx);

            LinearLayout left = Ui.column(ctx);
            String title = titleFor(o);
            String sub = subtitle(o);
            left.addView(Ui.text(ctx, title.isEmpty() ? "(unnamed)" : title, 14.5f, Ui.INK, true));
            if (!sub.isEmpty()) {
                left.addView(Ui.spacer(ctx, 2));
                left.addView(Ui.text(ctx, sub, 12, Ui.MUTED, false));
            }
            top.addView(left, weight(1f));

            String right = amount(o);
            if (!right.isEmpty()) {
                TextView r = Ui.text(ctx, right, 13.5f, Ui.BRAND, true);
                r.setGravity(Gravity.END);
                top.addView(r);
            }
            card.addView(top);

            String badge = badge(o);
            if (!badge.isEmpty()) {
                TextView b = Ui.text(ctx, badge, 10.5f, Ui.MUTED, false);
                b.setPadding(0, Ui.dp(ctx, 6), 0, 0);
                card.addView(b);
            }
            return card;
        }

        private String subtitle(JSONObject o) {
            List<String> parts = new ArrayList<>();
            if ("party".equals(kind)) {
                push(parts, first(o, "contact", "email", "phone", "city"));
            } else if ("product".equals(kind)) {
                push(parts, first(o, "sku"));
                push(parts, first(o, "category"));
                String stock = o.isNull("qty_on_hand") ? "" : Ui.qty(o.optDouble("qty_on_hand")) + " " + o.optString("unit");
                push(parts, stock.trim());
            } else if ("payment".equals(kind)) {
                push(parts, first(o, "party_name", "invoice_number"));
                push(parts, first(o, "date"));
                push(parts, first(o, "method"));
            } else {
                push(parts, first(o, "party_name", "supplier_name", "customer_name"));
                push(parts, first(o, "invoice_date", "order_date", "quote_date", "date"));
                push(parts, first(o, "due_date", "expected_date", "valid_until"));
            }
            StringBuilder sb = new StringBuilder();
            for (String s : parts) {
                if (sb.length() > 0) {
                    sb.append("  ·  ");
                }
                sb.append(s);
            }
            return sb.toString();
        }

        private String amount(JSONObject o) {
            JSONObject totals = o.optJSONObject("totals");
            if (totals != null && !totals.isNull("total")) {
                return Ui.money(totals.optDouble("total", 0), api.currency());
            }
            if (!o.isNull("amount")) {
                return Ui.money(o.optDouble("amount", 0), api.currency());
            }
            if (!o.isNull("qty_on_hand")) {
                return Ui.qty(o.optDouble("qty_on_hand", 0)) + " " + o.optString("unit");
            }
            return "";
        }

        private String badge(JSONObject o) {
            List<String> parts = new ArrayList<>();
            String status = o.optString("status", "");
            if (!status.isEmpty()) {
                parts.add(status.replace('_', ' '));
            }
            if (o.has("active")) {
                parts.add(o.optBoolean("active", true) ? "active" : "inactive");
            }
            if (!o.isNull("price") && !o.isNull("qty_on_hand")) {
                parts.add("price " + Ui.money(o.optDouble("price", 0), api.currency()));
            }
            StringBuilder sb = new StringBuilder();
            for (String part : parts) {
                if (sb.length() > 0) {
                    sb.append("  ·  ");
                }
                sb.append(part);
            }
            return sb.toString();
        }

        private void push(List<String> list, String value) {
            if (value != null) {
                String v = value.trim();
                if (!v.isEmpty()) {
                    list.add(v);
                }
            }
        }

        /** A document leads with its number, a party with its name, stock with the item's name. */
        private String titleFor(JSONObject o) {
            if ("product".equals(kind)) {
                return first(o, "name", "sku");
            }
            return first(o, "number", "code", "name");
        }

        private String first(JSONObject o, String... keys) {
            for (String k : keys) {
                String v = o.optString(k, "");
                if (!v.trim().isEmpty()) {
                    return v;
                }
            }
            return "";
        }
    }
}
