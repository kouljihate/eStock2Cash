package ma.s2c.maroc;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.Context;
import android.os.Bundle;
import android.text.Editable;
import android.text.InputType;
import android.text.TextWatcher;
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
import android.widget.Toast;

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

    /** The tabs along the bottom, in the order the shop reads them. */
    private static final int TAB_CASHIER = 0;
    private static final int TAB_PRODUCTS = 1;
    private static final int TAB_SUPPLIERS = 2;
    private static final int TAB_CUSTOMERS = 3;
    private static final int TAB_CREDIT = 4;

    private Api api;
    private Config cfg;
    private LocalDb local;
    private FrameLayout host;
    private final ArrayDeque<Screen> stack = new ArrayDeque<>();
    private int tab = TAB_CASHIER;

    /**
     * The till's cart, kept here rather than in a screen so that walking to Products to check
     * a price does not throw away what the cashier has already rung up.
     */
    private final java.util.LinkedHashMap<String, CartLine> cart = new java.util.LinkedHashMap<>();

    /** One product in the cart, with everything the receipt needs. */
    private static final class CartLine {
        String name;
        String unit;
        double price;
        double cost;
        double qty;
        double stock;
    }

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        cfg = new Config(this);
        local = new LocalDb(this);
        api = new Api(this, cfg);

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

        push(this::startScreen);
    }

    /**
     * Where the app opens: the sign-in page. Local mode has no account to ask for, so its card
     * just says where the data lives and opens the till; server mode asks for the username and
     * password. Either way the page carries the settings link and the version.
     */
    private View startScreen() {
        return loginScreen();
    }

    /** Past the login page: the cashier, with the four tabs under it. */
    private void openShop() {
        cart.clear();
        tab = TAB_CASHIER;
        stack.clear();
        push(this::shellScreen);
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

        if (cfg.isLocal()) {
            body.addView(localLoginCard());
        } else {
            body.addView(serverLoginCard());
        }

        TextView toSettings = Ui.text(this, "Settings  ·  الإعدادات  ·  v" + version(), 12.5f, Ui.BRAND, true);
        toSettings.setPadding(0, Ui.dp(this, 16), 0, 0);
        toSettings.setOnClickListener(v -> push(this::settingsScreen));
        body.addView(toSettings);

        scroll.addView(body);
        page.addView(scroll, Ui.lp(ViewGroup.LayoutParams.MATCH_PARENT, 0));
        ((LinearLayout.LayoutParams) scroll.getLayoutParams()).weight = 1;

        page.addView(footer());
        return page;
    }

    /** Local mode asks for nothing: it says where the data lives and opens the till. */
    private View localLoginCard() {
        LinearLayout card = Ui.card(this);
        card.addView(Ui.bi(this, "Welcome", "مرحباً", 18, Ui.INK, true));
        card.addView(Ui.spacer(this, 4));
        card.addView(Ui.text(this, cfg.company() + "  ·  " + cfg.currency(), 13, Ui.MUTED, false));
        card.addView(Ui.spacer(this, 14));

        JSONObject stats = local.stats();
        card.addView(Ui.bi(this, "Stored on this phone", "محفوظ على هذا الهاتف", 13, Ui.INK, true));
        card.addView(Ui.spacer(this, 4));
        card.addView(Ui.text(this, plural(stats.optInt("products"), "product") + "  ·  "
                + plural(stats.optInt("customers"), "customer") + "  ·  "
                + plural(stats.optInt("suppliers"), "supplier"), 12, Ui.MUTED, false));
        card.addView(Ui.spacer(this, 16));

        Button enter = bigButton("Enter  ·  دخول");
        enter.setOnClickListener(v -> openShop());
        card.addView(enter, Ui.lp(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        return card;
    }

    /** Server mode keeps the account it always had: the office database decides who may read. */
    private View serverLoginCard() {
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

        final Button go = bigButton("Sign in  ·  تسجيل الدخول");
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
                    openShop();
                }

                @Override
                public void fail(String message) {
                    go.setEnabled(true);
                    busy.setVisibility(View.GONE);
                    showError(error, message);
                }
            });
        });

        TextView hint = Ui.text(this, "Server mode reads the office computer's database, e.g. "
                + "http://192.168.1.20:8080. To work on the phone alone, switch Database to Local in Settings.",
                12, Ui.MUTED, false);
        hint.setPadding(0, Ui.dp(this, 12), 0, 0);
        card.addView(hint);
        return card;
    }

    /** The wide brand button every screen uses for its one main action. */
    private Button bigButton(String text) {
        Button b = new Button(this);
        b.setAllCaps(false);
        b.setText(Ui.font(this, text, true));
        b.setTextSize(14);
        b.setTextColor(Ui.WHITE);
        b.setBackground(Ui.rounded(Ui.BRAND, 12, this));
        b.setPadding(0, Ui.dp(this, 13), 0, Ui.dp(this, 13));
        return b;
    }

    /** "1 product", "3 products" — the counts on the sign-in card and the till header. */
    private String plural(int n, String word) {
        return n + " " + word + (n == 1 ? "" : "s");
    }

    private String version() {
        try {
            return getPackageManager().getPackageInfo(getPackageName(), 0).versionName;
        } catch (Exception e) {
            return "";
        }
    }

    private TextView fieldLabel(String latin, String arabic) {
        TextView t = Ui.text(this, latin + "  ·  " + arabic, 12, Ui.MUTED, false);
        t.setPadding(0, Ui.dp(this, 4), 0, Ui.dp(this, 5));
        return t;
    }

    private EditText field(String hint, String value, int variation) {
        EditText e = new EditText(this);
        // The input takes one face rather than spans: typing replaces spans as it goes, and
        // Arabic typed here still falls back to the system face glyph by glyph.
        e.setTypeface(Ui.latin(this, false));
        e.setHint(Ui.font(this, hint, false));
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

    /** The server dashboard, reachable from the cashier tab when the phone reads the office. */
    private View homeScreen() {
        return serverHomeScreen();
    }

    private View serverHomeScreen() {
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
        return gridCells(cells);
    }

    /** Pairs of cells, two to a row, with the gap that keeps the cards apart. */
    private View gridCells(String[][] cells) {
        LinearLayout grid = Ui.column(this);
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
        return serverListScreen(latin, arabic, path, kind);
    }

    private View serverListScreen(String latin, String arabic, String path, String kind) {
        LinearLayout page = Ui.column(this);
        page.addView(topBar(latin, arabic, true));
        page.addView(serverListBody(latin, arabic, path, kind));
        page.addView(footer());
        return page;
    }

    /** The list itself, without the bar or the footer: a tab's body, or the middle of a page. */
    private View serverListBody(String latin, String arabic, String path, String kind) {
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

        holder.setLayoutParams(Ui.lp(ViewGroup.LayoutParams.MATCH_PARENT, 0));
        ((LinearLayout.LayoutParams) holder.getLayoutParams()).weight = 1;
        return holder;
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
        again.setText(Ui.font(this, "Try again  ·  إعادة المحاولة", true));
        again.setTextSize(13);
        again.setTextColor(Ui.WHITE);
        again.setBackground(Ui.rounded(Ui.BRAND, 10, this));
        again.setOnClickListener(v -> retry.run());
        card.addView(again);
        return card;
    }

    private void showError(TextView target, String message) {
        target.setText(Ui.font(this, message, false));
        target.setVisibility(View.VISIBLE);
    }

    /* ------------------------------------------------------------- local mode */

    /** The list, read from the phone. Long-press a row to delete it. */
    private View localListBody(String latin, String arabic, String path, String kind) {
        LinearLayout page = Ui.column(this);

        JSONArray items = local.list(path, "");

        LinearLayout head = Ui.row(this);
        int h = Ui.dp(this, 12);
        head.setPadding(h, Ui.dp(this, 10), h, Ui.dp(this, 6));
        String count = items.length() + (items.length() == 1 ? " record  ·  سجل" : " records  ·  سجلات");
        head.addView(Ui.text(this, count, 12, Ui.MUTED, false), weight(1f));

        Button add = new Button(this);
        add.setAllCaps(false);
        add.setText(Ui.font(this, "+  Add  ·  إضافة", true));
        add.setTextSize(12.5f);
        add.setTextColor(Ui.WHITE);
        add.setBackground(Ui.rounded(Ui.BRAND, 10, this));
        add.setPadding(Ui.dp(this, 14), 0, Ui.dp(this, 14), 0);
        add.setOnClickListener(v -> push(() -> recordScreen(path, kind, new JSONObject())));
        head.addView(add);
        page.addView(head, Ui.lp(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));

        if (items.length() == 0) {
            LinearLayout empty = Ui.card(this);
            empty.addView(Ui.bi(this, "Nothing here yet.", "لا توجد سجلات بعد.", 14, Ui.MUTED, false));
            empty.addView(Ui.spacer(this, 6));
            empty.addView(Ui.text(this, "Tap Add to enter the first one — it is saved on this phone.", 12, Ui.MUTED, false));
            LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.WRAP_CONTENT);
            p.setMargins(Ui.dp(this, 12), Ui.dp(this, 4), Ui.dp(this, 12), 0);
            empty.setLayoutParams(p);
            page.addView(empty);
            page.addView(new View(this), verticalWeight());
        } else {
            ListView list = new ListView(this);
            list.setDivider(null);
            list.setDividerHeight(0);
            list.setPadding(Ui.dp(this, 12), Ui.dp(this, 6), Ui.dp(this, 12), Ui.dp(this, 6));
            list.setClipToPadding(false);
            list.setAdapter(new Rows(this, items, kind));
            list.setOnItemLongClickListener((parent, view, pos, id) -> {
                JSONObject row = items.optJSONObject(pos);
                if (row != null) {
                    confirmDelete(path, kind, latin, arabic, row);
                }
                return true;
            });
            page.addView(list, Ui.lp(ViewGroup.LayoutParams.MATCH_PARENT, 0));
            ((LinearLayout.LayoutParams) list.getLayoutParams()).weight = 1;
        }

        return page;
    }

    private void confirmDelete(String path, String kind, String latin, String arabic, JSONObject row) {
        String name = row.optString("name", "");
        AlertDialog dialog = new AlertDialog.Builder(this)
                .setTitle(Ui.font(this, "Delete  ·  حذف", true))
                .setMessage(Ui.font(this, name + "\n\nRemoves this record from the phone's database. لا يمكن التراجع بعد الحذف.", false))
                .setNegativeButton(Ui.font(this, "Cancel  ·  إلغاء", false), null)
                .setPositiveButton(Ui.font(this, "Delete  ·  حذف", true), (d, which) -> {
                    local.remove(path, row.optString("id", ""));
                    selectTab(tab);
                })
                .create();
        dialog.show();
        sentenceCase(dialog);
    }

    /** Android shouts dialog buttons in capitals; every label in this app is sentence case. */
    private void sentenceCase(AlertDialog dialog) {
        int[] buttons = {AlertDialog.BUTTON_POSITIVE, AlertDialog.BUTTON_NEGATIVE, AlertDialog.BUTTON_NEUTRAL};
        for (int which : buttons) {
            Button button = dialog.getButton(which);
            if (button != null) {
                button.setAllCaps(false);
                button.setText(button.getText());
            }
        }
    }

    /** The form for one supplier, customer or product, saved into the phone's database. */
    private View recordScreen(String path, String kind, JSONObject existing) {
        boolean product = path.endsWith("products");
        boolean customer = path.endsWith("customers");
        String latin = product ? "Product" : customer ? "Customer" : "Supplier";
        String arabic = product ? "منتج" : customer ? "عميل" : "مورد";
        boolean edit = !existing.optString("id", "").isEmpty();

        LinearLayout page = Ui.column(this);
        page.addView(topBar(latin, arabic, true));

        ScrollView scroll = new ScrollView(this);
        LinearLayout body = Ui.column(this);
        int b = Ui.dp(this, 14);
        body.setPadding(b, b, b, b);

        LinearLayout card = Ui.card(this);
        card.addView(Ui.bi(this, (edit ? "Edit " : "New ") + latin.toLowerCase(java.util.Locale.US),
                edit ? "تعديل" : "جديد", 16, Ui.INK, true));
        card.addView(Ui.spacer(this, 12));

        final java.util.LinkedHashMap<String, EditText> values = new java.util.LinkedHashMap<>();
        values.put("name", addField(card, "Name", "الاسم", existing.optString("name", "")));
        if (product) {
            values.put("sku", addField(card, "SKU", "الرمز", existing.optString("sku", "")));
            values.put("category", addField(card, "Category", "الصنف", existing.optString("category", "")));
            values.put("unit", addField(card, "Unit", "الوحدة", existing.optString("unit", "pc")));
            values.put("cost", addNumberField(card, "Cost", "سعر الشراء", zero(existing, "cost")));
            values.put("price", addNumberField(card, "Price", "سعر البيع", zero(existing, "price")));
            values.put("qty_on_hand", addNumberField(card, "Stock", "المخزون", zero(existing, "qty_on_hand")));
            values.put("reorder_level", addNumberField(card, "Reorder level", "حد الطلب", zero(existing, "reorder_level")));
        } else {
            values.put("code", addField(card, "Code", "الرمز", existing.optString("code", "")));
            values.put("contact", addField(card, "Contact", "المسؤول", existing.optString("contact", "")));
            values.put("phone", addField(card, "Phone", "الهاتف", existing.optString("phone", "")));
            values.put("email", addField(card, "Email", "البريد", existing.optString("email", "")));
            values.put("city", addField(card, "City", "المدينة", existing.optString("city", "")));
            values.put("address", addField(card, "Address", "العنوان", existing.optString("address", "")));
            if (customer) {
                values.put("credit_limit", addNumberField(card, "Credit limit", "سقف الدين", zero(existing, "credit_limit")));
            }
        }

        final TextView error = Ui.text(this, "", 12.5f, Ui.RED, false);
        error.setVisibility(View.GONE);
        card.addView(error);

        Button save = new Button(this);
        save.setAllCaps(false);
        save.setText(Ui.font(this, "Save  ·  حفظ", true));
        save.setTextSize(14);
        save.setTextColor(Ui.WHITE);
        save.setBackground(Ui.rounded(Ui.BRAND, 12, this));
        save.setPadding(0, Ui.dp(this, 13), 0, Ui.dp(this, 13));
        save.setOnClickListener(v -> {
            JSONObject row = new JSONObject();
            try {
                row.put("id", existing.optString("id", ""));
                for (java.util.Map.Entry<String, EditText> e : values.entrySet()) {
                    row.put(e.getKey(), e.getValue().getText().toString().trim());
                }
            } catch (Exception ignored) {
                // Strings into a JSONObject cannot fail this way.
            }
            String problem = local.save(path, row);
            if (problem != null) {
                showError(error, problem);
                return;
            }
            String saved = product ? "Product" : customer ? "Customer" : "Supplier";
            selectTab(tab);
            Toast.makeText(this, saved + " saved  ·  تم الحفظ", Toast.LENGTH_SHORT).show();
        });
        card.addView(save, Ui.lp(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));

        body.addView(card);
        scroll.addView(body);
        page.addView(scroll, Ui.lp(ViewGroup.LayoutParams.MATCH_PARENT, 0));
        ((LinearLayout.LayoutParams) scroll.getLayoutParams()).weight = 1;

        page.addView(footer());
        return page;
    }

    private String zero(JSONObject row, String key) {
        double v = row.optDouble(key, 0);
        return v == 0 ? "0" : Ui.qty(v);
    }

    private EditText addField(LinearLayout card, String latin, String arabic, String value) {
        card.addView(fieldLabel(latin, arabic));
        EditText e = field(latin, value, InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS);
        card.addView(e);
        card.addView(Ui.spacer(this, 8));
        return e;
    }

    private EditText addNumberField(LinearLayout card, String latin, String arabic, String value) {
        EditText e = addField(card, latin, arabic, value);
        e.setInputType(InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_FLAG_DECIMAL);
        return e;
    }

    /* -------------------------------------------------------------- settings */

    /**
     * The two database choices and the addresses that go with them. Everything here is
     * written to s2c.properties, so it can also be changed with a text editor later.
     */
    private View settingsScreen() {
        LinearLayout page = Ui.column(this);
        page.addView(topBar("Settings", "الإعدادات", true));

        ScrollView scroll = new ScrollView(this);
        LinearLayout body = Ui.column(this);
        int b = Ui.dp(this, 14);
        body.setPadding(b, b, b, b);

        final String[] mode = {cfg.mode()};

        LinearLayout card = Ui.card(this);
        card.addView(Ui.bi(this, "Database", "قاعدة البيانات", 16, Ui.INK, true));
        card.addView(Ui.spacer(this, 6));
        card.addView(Ui.text(this, "Local keeps everything on this phone and needs no server. "
                + "Server reads the office computer over the LAN.", 12, Ui.MUTED, false));
        card.addView(Ui.spacer(this, 12));

        LinearLayout choices = Ui.row(this);
        Button localBtn = choiceButton("Local  ·  هذا الهاتف", Config.MODE_LOCAL.equals(mode[0]));
        Button serverBtn = choiceButton("Server  ·  الخادم", Config.MODE_SERVER.equals(mode[0]));
        localBtn.setOnClickListener(v -> {
            mode[0] = Config.MODE_LOCAL;
            stack.pop();
            push(this::settingsScreen);
        });
        serverBtn.setOnClickListener(v -> {
            mode[0] = Config.MODE_SERVER;
            stack.pop();
            push(this::settingsScreen);
        });
        choices.addView(localBtn, weighted());
        choices.addView(serverBtn, weightedWithGap());
        card.addView(choices);
        card.addView(Ui.spacer(this, 12));

        card.addView(fieldLabel("Server address", "عنوان الخادم"));
        final EditText server = field("http://192.168.1.20:8080", cfg.server(), InputType.TYPE_TEXT_VARIATION_URI);
        card.addView(server);
        card.addView(Ui.spacer(this, 10));

        card.addView(fieldLabel("Company", "الشركة"));
        final EditText company = field("S2C MAROC", cfg.company(), InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS);
        card.addView(company);
        card.addView(Ui.spacer(this, 10));

        card.addView(fieldLabel("Currency", "العملة"));
        final EditText currency = field("MAD", cfg.currency(), InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS);
        card.addView(currency);
        card.addView(Ui.spacer(this, 14));

        final TextView error = Ui.text(this, "", 12.5f, Ui.RED, false);
        error.setVisibility(View.GONE);
        card.addView(error);

        Button save = new Button(this);
        save.setAllCaps(false);
        save.setText(Ui.font(this, "Save  ·  حفظ", true));
        save.setTextSize(14);
        save.setTextColor(Ui.WHITE);
        save.setBackground(Ui.rounded(Ui.BRAND, 12, this));
        save.setPadding(0, Ui.dp(this, 13), 0, Ui.dp(this, 13));
        save.setOnClickListener(v -> {
            String url = server.getText().toString().trim();
            if (Config.MODE_SERVER.equals(mode[0]) && url.isEmpty()) {
                showError(error, "Enter the server address, e.g. http://192.168.1.20:8080");
                return;
            }
            String oldServer = cfg.server();
            cfg.setMode(mode[0]);
            cfg.setServer(url);
            cfg.setCompany(company.getText().toString());
            cfg.setCurrency(currency.getText().toString());
            String problem = cfg.save();
            if (problem != null) {
                showError(error, "Could not write " + cfg.path() + ": " + problem);
                return;
            }
            if (!oldServer.equals(cfg.server())) {
                // Another server means another account: the old token is worthless.
                api.signOut();
            }
            stack.clear();
            push(MainActivity.this::startScreen);
        });
        card.addView(save, Ui.lp(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        body.addView(card);
        body.addView(Ui.spacer(this, 12));

        LinearLayout fileCard = Ui.card(this);
        fileCard.addView(Ui.bi(this, "Settings file", "ملف الإعدادات", 13, Ui.INK, true));
        fileCard.addView(Ui.spacer(this, 6));
        fileCard.addView(Ui.text(this, "These settings are kept in a text file, so they can be changed "
                + "later from a computer as well as from this screen:", 12, Ui.MUTED, false));
        fileCard.addView(Ui.spacer(this, 6));
        TextView path = Ui.text(this, cfg.path(), 11, Ui.BRAND, false);
        path.setTextIsSelectable(true);
        fileCard.addView(path);
        body.addView(fileCard);

        scroll.addView(body);
        page.addView(scroll, Ui.lp(ViewGroup.LayoutParams.MATCH_PARENT, 0));
        ((LinearLayout.LayoutParams) scroll.getLayoutParams()).weight = 1;

        page.addView(footer());
        return page;
    }

    private Button choiceButton(String text, boolean selected) {
        Button b = new Button(this);
        b.setAllCaps(false);
        b.setText(Ui.font(this, text, true));
        b.setTextSize(12.5f);
        paintChoice(b, selected);
        return b;
    }

    private void paintChoice(Button b, boolean selected) {
        b.setTextColor(selected ? Ui.WHITE : Ui.INK);
        b.setBackground(selected
                ? Ui.rounded(Ui.BRAND, 10, this)
                : Ui.outlined(Ui.WHITE, 10, Ui.LINE, this));
    }

    /* ----------------------------------------------------------- shell and tabs */

    /** Every tab in one page: the navy bar, the body, the strip along the bottom, the footer. */
    private View shellScreen() {
        LinearLayout page = Ui.column(this);
        page.addView(topBar(tabLatin(), tabArabic(), false));

        View body;
        if (tab == TAB_PRODUCTS) {
            body = cfg.isLocal()
                    ? localListBody("Products", "المنتجات", "/products", "product")
                    : serverListBody("Products", "المنتجات", "/products", "product");
        } else if (tab == TAB_SUPPLIERS) {
            body = cfg.isLocal()
                    ? localListBody("Suppliers", "الموردون", "/suppliers", "party")
                    : serverListBody("Suppliers", "الموردون", "/suppliers", "party");
        } else if (tab == TAB_CUSTOMERS) {
            body = cfg.isLocal()
                    ? localListBody("Customers", "العملاء", "/customers", "party")
                    : serverListBody("Customers", "العملاء", "/customers", "party");
        } else if (tab == TAB_CREDIT) {
            body = creditBody();
        } else {
            body = cfg.isLocal() ? cashierBody() : needsLocalBody("Cashier", "الصندوق");
        }
        page.addView(body, verticalWeight());
        page.addView(tabBar());
        page.addView(footer());
        return page;
    }

    private String tabLatin() {
        switch (tab) {
            case TAB_PRODUCTS:
                return "Products";
            case TAB_SUPPLIERS:
                return "Suppliers";
            case TAB_CUSTOMERS:
                return "Customers";
            case TAB_CREDIT:
                return "Credit";
            default:
                return "Cashier";
        }
    }

    private String tabArabic() {
        switch (tab) {
            case TAB_PRODUCTS:
                return "المنتجات";
            case TAB_SUPPLIERS:
                return "الموردون";
            case TAB_CUSTOMERS:
                return "العملاء";
            case TAB_CREDIT:
                return "الائتمان";
            default:
                return "الصندوق";
        }
    }

    /** A tab is a fresh root: the back key leaves the shop, it does not walk back through tabs. */
    private void selectTab(int index) {
        tab = index;
        stack.clear();
        stack.push(this::shellScreen);
        render();
    }

    private View tabBar() {
        String[][] tabs = {
                {"Cashier", "الصندوق"},
                {"Products", "المنتجات"},
                {"Suppliers", "الموردون"},
                {"Customers", "العملاء"},
                {"Credit", "الائتمان"},
        };
        LinearLayout bar = Ui.row(this);
        bar.setBackgroundColor(Ui.WHITE);
        bar.setPadding(Ui.dp(this, 2), Ui.dp(this, 6), Ui.dp(this, 2), Ui.dp(this, 6));

        for (int i = 0; i < tabs.length; i++) {
            boolean on = i == tab;
            LinearLayout cell = Ui.column(this);
            cell.setGravity(Gravity.CENTER);
            cell.setPadding(0, Ui.dp(this, 4), 0, Ui.dp(this, 4));
            if (on) {
                cell.setBackground(Ui.rounded(Ui.BRAND_SOFT, 10, this));
            }
            TextView arabic = Ui.text(this, tabs[i][1], 10, on ? Ui.BRAND : Ui.MUTED, on);
            arabic.setGravity(Gravity.CENTER);
            TextView latin = Ui.text(this, tabs[i][0], 9.5f, on ? Ui.BRAND : Ui.MUTED, on);
            latin.setGravity(Gravity.CENTER);
            cell.addView(arabic);
            cell.addView(latin);
            final int index = i;
            cell.setOnClickListener(v -> selectTab(index));
            bar.addView(cell, weight(1f));
        }
        return bar;
    }

    /** The till and the credit book need the phone's own data; say so instead of faking one. */
    private View needsLocalBody(String latin, String arabic) {
        LinearLayout page = Ui.column(this);
        LinearLayout card = Ui.card(this);
        card.setLayoutParams(cardParams());
        card.addView(Ui.bi(this, latin, arabic, 16, Ui.INK, true));
        card.addView(Ui.spacer(this, 6));
        card.addView(Ui.text(this, "This phone is set to Server, so there is no till here. "
                + "Switch Database to Local in Settings to sell from this phone.", 12, Ui.MUTED, false));
        card.addView(Ui.spacer(this, 12));
        Button settings = bigButton("Settings  ·  الإعدادات");
        settings.setOnClickListener(v -> push(this::settingsScreen));
        card.addView(settings);
        card.addView(Ui.spacer(this, 6));
        TextView dashboard = Ui.text(this, "Open the server dashboard  ·  لوحة التحكم", 12, Ui.BRAND, true);
        dashboard.setPadding(0, Ui.dp(this, 8), 0, 0);
        dashboard.setOnClickListener(v -> push(this::homeScreen));
        card.addView(dashboard);
        page.addView(card);
        page.addView(new View(this), verticalWeight());
        return page;
    }

    /** A child that fills what is left of a column, the way weight does in a row. */
    private LinearLayout.LayoutParams verticalWeight() {
        return new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f);
    }

    private LinearLayout.LayoutParams cardParams() {
        LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT);
        p.setMargins(Ui.dp(this, 12), Ui.dp(this, 12), Ui.dp(this, 12), 0);
        return p;
    }

    /* ----------------------------------------------------------------- the till */

    /** The cashier: find a product, tap it, ring it up. The cart survives a look at another tab. */
    private View cashierBody() {
        LinearLayout page = Ui.column(this);

        JSONObject stats = local.stats();
        LinearLayout head = Ui.card(this);
        head.setPadding(Ui.dp(this, 12), Ui.dp(this, 12), Ui.dp(this, 12), Ui.dp(this, 12));
        head.setLayoutParams(cardParams());
        head.addView(Ui.text(this, plural(stats.optInt("products"), "product") + "  ·  "
                + stats.optInt("low_stock") + " low stock", 11.5f, Ui.MUTED, false));
        head.addView(Ui.spacer(this, 8));
        final EditText search = field("Find a product · ابحث عن منتج", "", InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS);
        head.addView(search);
        page.addView(head);

        final LinearLayout results = Ui.column(this);
        ScrollView scroll = new ScrollView(this);
        scroll.addView(results);
        page.addView(scroll, verticalWeight());

        final LinearLayout strip = Ui.card(this);
        strip.setPadding(Ui.dp(this, 14), Ui.dp(this, 12), Ui.dp(this, 14), Ui.dp(this, 12));
        strip.setLayoutParams(cardParams());
        page.addView(strip);

        fillStrip(strip);
        fillResults(results, "", strip);

        search.addTextChangedListener(new TextWatcher() {
            @Override
            public void beforeTextChanged(CharSequence s, int start, int count, int after) {
            }

            @Override
            public void onTextChanged(CharSequence s, int start, int before, int count) {
            }

            @Override
            public void afterTextChanged(Editable s) {
                fillResults(results, s == null ? "" : s.toString(), strip);
            }
        });
        return page;
    }

    private void fillResults(LinearLayout holder, String q, LinearLayout strip) {
        holder.removeAllViews();
        JSONArray items = local.list("/products", q);
        if (items.length() == 0) {
            LinearLayout card = Ui.card(this);
            card.setLayoutParams(cardParams());
            card.addView(Ui.bi(this, q.isEmpty() ? "No products yet" : "Nothing matches",
                    q.isEmpty() ? "لا توجد منتجات بعد" : "لا يوجد تطابق", 13, Ui.MUTED, false));
            card.addView(Ui.spacer(this, 6));
            card.addView(Ui.text(this, "Add them in the Products tab, then come back here.", 12, Ui.MUTED, false));
            holder.addView(card);
            return;
        }
        for (int i = 0; i < items.length(); i++) {
            JSONObject row = items.optJSONObject(i);
            if (row != null) {
                holder.addView(productRow(row, strip));
            }
        }
    }

    private View productRow(JSONObject o, LinearLayout strip) {
        LinearLayout card = Ui.card(this);
        card.setPadding(Ui.dp(this, 14), Ui.dp(this, 11), Ui.dp(this, 14), Ui.dp(this, 11));
        LinearLayout row = Ui.row(this);

        LinearLayout left = Ui.column(this);
        left.addView(Ui.text(this, o.optString("name", "(unnamed)"), 14, Ui.INK, true));
        StringBuilder sub = new StringBuilder();
        sub.append(Ui.qty(o.optDouble("qty_on_hand", 0))).append(' ').append(o.optString("unit", "pc"));
        if (!o.optString("sku", "").trim().isEmpty()) {
            sub.append("  ·  ").append(o.optString("sku"));
        }
        left.addView(Ui.text(this, sub.toString(), 11.5f, Ui.MUTED, false));
        row.addView(left, weight(1f));

        TextView price = Ui.text(this, Ui.money(o.optDouble("price", 0), api.currency()), 13.5f, Ui.BRAND, true);
        price.setGravity(Gravity.END);
        row.addView(price);
        card.addView(row);

        card.setOnClickListener(v -> addToCart(o, strip));
        LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT);
        p.setMargins(Ui.dp(this, 12), Ui.dp(this, 6), Ui.dp(this, 12), 0);
        card.setLayoutParams(p);
        return card;
    }

    private void addToCart(JSONObject o, LinearLayout strip) {
        String id = o.optString("id", "");
        double stock = o.optDouble("qty_on_hand", 0);
        CartLine line = cart.get(id);
        if (stock <= 0) {
            Toast.makeText(this, "Out of stock  ·  نفد المخزون", Toast.LENGTH_SHORT).show();
            return;
        }
        if (line != null && line.qty + 1 > stock) {
            Toast.makeText(this, "Only " + Ui.qty(stock) + " in stock  ·  المخزون لا يكفي", Toast.LENGTH_SHORT).show();
            return;
        }
        if (line == null) {
            line = new CartLine();
            line.name = o.optString("name", "");
            line.unit = o.optString("unit", "pc");
            line.price = o.optDouble("price", 0);
            line.cost = o.optDouble("cost", 0);
            line.stock = stock;
            cart.put(id, line);
        }
        line.qty += 1;
        fillStrip(strip);
    }

    /** The bar under the products: what is in the cart, what it costs, and the way to pay. */
    private void fillStrip(LinearLayout strip) {
        strip.removeAllViews();
        LinearLayout row = Ui.row(this);
        row.addView(Ui.bi(this, cart.size() + (cart.size() == 1 ? " line" : " lines") + "  ·  " + Ui.qty(units()) + " units",
                "في السلة", 12, Ui.MUTED, false), weight(1f));
        row.addView(Ui.text(this, Ui.money(total(), api.currency()), 17, Ui.INK, true));
        strip.addView(row);
        strip.addView(Ui.spacer(this, 10));

        Button review = bigButton(cart.isEmpty() ? "Cart is empty  ·  السلة فارغة" : "Review cart  ·  مراجعة السلة");
        review.setEnabled(!cart.isEmpty());
        review.setOnClickListener(v -> push(this::cartScreen));
        strip.addView(review, Ui.lp(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
    }

    private double units() {
        double sum = 0;
        for (CartLine line : cart.values()) {
            sum += line.qty;
        }
        return sum;
    }

    private double total() {
        double sum = 0;
        for (CartLine line : cart.values()) {
            sum += line.qty * line.price;
        }
        return sum;
    }

    /* ----------------------------------------------------------------- the cart */

    /** The cart: change a quantity, drop a line, then take the money or put it on credit. */
    private View cartScreen() {
        LinearLayout page = Ui.column(this);
        page.addView(topBar("Cart", "السلة", true));

        ScrollView scroll = new ScrollView(this);
        LinearLayout body = Ui.column(this);
        int b = Ui.dp(this, 12);
        body.setPadding(b, b, b, b);

        final LinearLayout lines = Ui.column(this);
        body.addView(lines);

        final TextView error = Ui.text(this, "", 12.5f, Ui.RED, false);
        error.setVisibility(View.GONE);

        final String[] payment = {"cash"};
        final String[] customerId = {""};
        final String[] customerName = {""};

        LinearLayout money = Ui.card(this);
        money.setLayoutParams(cardParams());
        money.addView(Ui.bi(this, "Total", "المجموع", 12, Ui.MUTED, false));
        money.addView(Ui.spacer(this, 4));
        final TextView totalText = Ui.text(this, Ui.money(total(), api.currency()), 22, Ui.INK, true);
        money.addView(totalText);
        money.addView(Ui.spacer(this, 12));

        money.addView(Ui.bi(this, "Payment", "الدفع", 13, Ui.INK, true));
        money.addView(Ui.spacer(this, 8));
        LinearLayout choices = Ui.row(this);
        final Button cash = choiceButton("Cash  ·  نقداً", true);
        final Button credit = choiceButton("Credit  ·  بالأجل", false);
        choices.addView(cash, weighted());
        choices.addView(credit, weightedWithGap());
        money.addView(choices);

        final TextView pickCustomer = Ui.text(this, "Choose the customer  ·  اختر العميل", 12.5f, Ui.BRAND, true);
        pickCustomer.setPadding(0, Ui.dp(this, 12), 0, 0);
        pickCustomer.setVisibility(View.GONE);
        pickCustomer.setOnClickListener(v -> {
            JSONArray people = local.list("/customers", "");
            if (people.length() == 0) {
                Toast.makeText(this, "Add a customer first  ·  أضف عميلاً أولاً", Toast.LENGTH_SHORT).show();
                return;
            }
            final String[] names = new String[people.length()];
            for (int i = 0; i < people.length(); i++) {
                JSONObject person = people.optJSONObject(i);
                names[i] = person == null ? "" : person.optString("name", "");
            }
            new AlertDialog.Builder(this)
                    .setTitle(Ui.font(this, "Who takes the credit?  ·  من عليه الأجل؟", true))
                    .setItems(names, (dialog, which) -> {
                        JSONObject person = people.optJSONObject(which);
                        customerId[0] = person == null ? "" : person.optString("id", "");
                        customerName[0] = names[which];
                        pickCustomer.setText(Ui.font(this, "Credit to  ·  بالأجل على " + customerName[0], true));
                    })
                    .show();
        });
        money.addView(pickCustomer);

        cash.setOnClickListener(v -> {
            payment[0] = "cash";
            customerId[0] = "";
            customerName[0] = "";
            paintChoice(cash, true);
            paintChoice(credit, false);
            pickCustomer.setVisibility(View.GONE);
        });
        credit.setOnClickListener(v -> {
            payment[0] = "credit";
            paintChoice(cash, false);
            paintChoice(credit, true);
            pickCustomer.setVisibility(View.VISIBLE);
        });

        money.addView(Ui.spacer(this, 14));
        money.addView(error);
        Button charge = bigButton("Charge  ·  تحصيل");
        charge.setOnClickListener(v -> {
            if (cart.isEmpty()) {
                showError(error, "The cart is empty.");
                return;
            }
            if ("credit".equals(payment[0]) && customerId[0].isEmpty()) {
                showError(error, "Choose the customer who takes the credit.");
                return;
            }
            JSONArray linesOut = new JSONArray();
            try {
                for (java.util.Map.Entry<String, CartLine> e : cart.entrySet()) {
                    CartLine line = e.getValue();
                    JSONObject item = new JSONObject();
                    item.put("product_id", e.getKey());
                    item.put("name", line.name);
                    item.put("qty", line.qty);
                    item.put("price", line.price);
                    item.put("cost", line.cost);
                    linesOut.put(item);
                }
                String number = local.nextNumber();
                JSONObject sale = new JSONObject();
                sale.put("number", number);
                sale.put("payment", payment[0]);
                sale.put("customer_id", customerId[0]);
                sale.put("customer_name", customerName[0]);
                String problem = local.saveSale(sale, linesOut);
                if (problem != null) {
                    showError(error, problem);
                    return;
                }
                cart.clear();
                selectTab(TAB_CASHIER);
                Toast.makeText(this, "Sale " + number + " saved  ·  تم حفظ البيع", Toast.LENGTH_LONG).show();
            } catch (Exception e) {
                showError(error, "Could not save the sale: " + e.getMessage());
            }
        });
        money.addView(charge, Ui.lp(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        body.addView(money);

        renderCartLines(lines, totalText);

        scroll.addView(body);
        page.addView(scroll, Ui.lp(ViewGroup.LayoutParams.MATCH_PARENT, 0));
        ((LinearLayout.LayoutParams) scroll.getLayoutParams()).weight = 1;
        page.addView(footer());
        return page;
    }

    private void renderCartLines(LinearLayout holder, final TextView totalText) {
        holder.removeAllViews();
        if (cart.isEmpty()) {
            LinearLayout card = Ui.card(this);
            card.setLayoutParams(cardParams());
            card.addView(Ui.bi(this, "The cart is empty", "السلة فارغة", 13, Ui.MUTED, false));
            holder.addView(card);
            return;
        }
        for (java.util.Map.Entry<String, CartLine> entry : cart.entrySet()) {
            final String id = entry.getKey();
            final CartLine line = entry.getValue();

            LinearLayout card = Ui.card(this);
            card.setPadding(Ui.dp(this, 14), Ui.dp(this, 11), Ui.dp(this, 14), Ui.dp(this, 11));
            card.setLayoutParams(cardParams());

            LinearLayout top = Ui.row(this);
            LinearLayout left = Ui.column(this);
            left.addView(Ui.text(this, line.name, 13.5f, Ui.INK, true));
            left.addView(Ui.text(this, Ui.money(line.price, api.currency()) + "  ×  " + Ui.qty(line.qty),
                    11.5f, Ui.MUTED, false));
            top.addView(left, weight(1f));
            top.addView(Ui.text(this, Ui.money(line.price * line.qty, api.currency()), 13, Ui.BRAND, true));
            card.addView(top);

            LinearLayout controls = Ui.row(this);
            controls.setPadding(0, Ui.dp(this, 8), 0, 0);
            TextView minus = stepChip("−");
            TextView qty = Ui.text(this, Ui.qty(line.qty) + " " + line.unit, 12.5f, Ui.INK, true);
            qty.setPadding(Ui.dp(this, 12), 0, Ui.dp(this, 12), 0);
            TextView plus = stepChip("+");
            controls.addView(minus);
            controls.addView(qty);
            controls.addView(plus);
            controls.addView(new View(this), weight(1f));
            TextView remove = Ui.text(this, "Remove  ·  إزالة", 11.5f, Ui.RED, false);
            controls.addView(remove);
            card.addView(controls);

            minus.setOnClickListener(v -> {
                if (line.qty > 1) {
                    line.qty -= 1;
                } else {
                    cart.remove(id);
                }
                renderCartLines(holder, totalText);
                totalText.setText(Ui.font(this, Ui.money(total(), api.currency()), true));
            });
            plus.setOnClickListener(v -> {
                if (line.qty + 1 > line.stock) {
                    Toast.makeText(this, "Only " + Ui.qty(line.stock) + " in stock  ·  المخزون لا يكفي",
                            Toast.LENGTH_SHORT).show();
                    return;
                }
                line.qty += 1;
                renderCartLines(holder, totalText);
                totalText.setText(Ui.font(this, Ui.money(total(), api.currency()), true));
            });
            remove.setOnClickListener(v -> {
                cart.remove(id);
                renderCartLines(holder, totalText);
                totalText.setText(Ui.font(this, Ui.money(total(), api.currency()), true));
            });
            holder.addView(card);
        }
    }

    /** A small round button for the cart's minus and plus. */
    private TextView stepChip(String symbol) {
        TextView t = Ui.text(this, symbol, 16, Ui.BRAND, true);
        t.setGravity(Gravity.CENTER);
        int size = Ui.dp(this, 32);
        t.setLayoutParams(new LinearLayout.LayoutParams(size, size));
        t.setBackground(Ui.rounded(Ui.BRAND_SOFT, 16, this));
        return t;
    }

    /* ----------------------------------------------------------------- the credit */

    /** Who owes the shop money, and one long press to say they have paid. */
    private View creditBody() {
        if (!cfg.isLocal()) {
            return needsLocalBody("Credit", "الائتمان");
        }
        LinearLayout page = Ui.column(this);
        JSONObject data = local.credit();

        LinearLayout summary = Ui.card(this);
        summary.setLayoutParams(cardParams());
        summary.addView(Ui.bi(this, "Owed to the shop", "المستحق لنا", 12, Ui.MUTED, false));
        summary.addView(Ui.spacer(this, 6));
        summary.addView(Ui.text(this, Ui.money(data.optDouble("total", 0), api.currency()), 22, Ui.INK, true));
        summary.addView(Ui.spacer(this, 4));
        summary.addView(Ui.text(this, plural(data.optInt("count"), "customer")
                + " with an open credit sale  ·  اضغط مطولاً لسداد", 11.5f, Ui.MUTED, false));
        page.addView(summary);

        JSONArray rows = data.optJSONArray("items");
        if (rows == null || rows.length() == 0) {
            LinearLayout empty = Ui.card(this);
            empty.setLayoutParams(cardParams());
            empty.addView(Ui.bi(this, "Nobody owes anything", "لا توجد ديون", 13, Ui.INK, true));
            empty.addView(Ui.spacer(this, 6));
            empty.addView(Ui.text(this, "A sale taken as Credit at the till turns up here until it is settled.",
                    12, Ui.MUTED, false));
            page.addView(empty);
            page.addView(new View(this), verticalWeight());
            return page;
        }

        ListView list = new ListView(this);
        list.setDivider(null);
        list.setDividerHeight(0);
        list.setPadding(Ui.dp(this, 12), Ui.dp(this, 6), Ui.dp(this, 12), Ui.dp(this, 6));
        list.setClipToPadding(false);
        list.setAdapter(new Rows(this, rows, "credit"));
        final JSONArray people = rows;
        list.setOnItemLongClickListener((parent, view, pos, id) -> {
            JSONObject person = people.optJSONObject(pos);
            if (person != null) {
                confirmSettle(person);
            }
            return true;
        });
        page.addView(list, Ui.lp(ViewGroup.LayoutParams.MATCH_PARENT, 0));
        ((LinearLayout.LayoutParams) list.getLayoutParams()).weight = 1;
        return page;
    }

    private void confirmSettle(JSONObject person) {
        String name = person.optString("name", "");
        String owed = Ui.money(person.optDouble("outstanding", 0), api.currency());
        AlertDialog dialog = new AlertDialog.Builder(this)
                .setTitle(Ui.font(this, "Settle the credit  ·  سداد الأجل", true))
                .setMessage(Ui.font(this, name + " owes " + owed
                        + ".\n\nThe sales are marked paid and leave the credit book. "
                        + "يتم تسجيل كل مبيعاته كمدفوعة.", false))
                .setNegativeButton(Ui.font(this, "Cancel  ·  إلغاء", false), null)
                .setPositiveButton(Ui.font(this, "Settle  ·  سداد", true), (d, which) -> {
                    local.settle(person.optString("id", ""));
                    selectTab(TAB_CREDIT);
                    Toast.makeText(this, name + " settled  ·  تم السداد", Toast.LENGTH_SHORT).show();
                })
                .create();
        dialog.show();
        sentenceCase(dialog);
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
        TextView action = Ui.text(this, cfg.isLocal() ? "Settings  ·  إعدادات" : "Sign out  ·  خروج",
                12, 0xFFC7D2FE, false);
        action.setOnClickListener(v -> {
            if (cfg.isLocal()) {
                push(this::settingsScreen);
                return;
            }
            api.signOut();
            stack.clear();
            push(this::startScreen);
        });
        right.addView(action);
        bar.addView(right);
        return bar;
    }

    private View footer() {
        String version = version();
        String where = cfg.isLocal() ? "local db" : "server";
        TextView t = Ui.text(this, cfg.company() + "  ·  v" + version + "  ·  " + where, 10.5f, Ui.MUTED, false);
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
                // The stock is the figure on the right, so it is not repeated here.
            } else if ("payment".equals(kind)) {
                push(parts, first(o, "party_name", "invoice_number"));
                push(parts, first(o, "date"));
                push(parts, first(o, "method"));
            } else if ("credit".equals(kind)) {
                push(parts, first(o, "phone"));
                push(parts, o.optInt("open_sales") + (o.optInt("open_sales") == 1 ? " open sale" : " open sales"));
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
            if (!o.isNull("outstanding")) {
                return Ui.money(o.optDouble("outstanding", 0), api.currency());
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
            if (!o.isNull("credit_limit") && o.optDouble("credit_limit", 0) > 0) {
                parts.add("limit " + Ui.money(o.optDouble("credit_limit", 0), api.currency()));
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
