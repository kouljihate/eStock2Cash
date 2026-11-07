package ma.s2c.maroc;

import android.content.Context;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;

/**
 * The look of the phone app, taken from the web UI: the same navy header, the same
 * indigo, the same white cards on a pale page. Two rules carried over deliberately —
 * the app is always left-to-right, and wherever a label has both languages the Arabic
 * sits above the Latin.
 */
final class Ui {

    static final int BG = 0xFFF4F6FB;
    static final int CARD = 0xFFFFFFFF;
    static final int INK = 0xFF0F172A;
    static final int MUTED = 0xFF64748B;
    static final int LINE = 0xFFE2E8F0;
    static final int BRAND = 0xFF4F46E5;
    static final int BRAND_SOFT = 0xFFEEF1FE;
    static final int DARK = 0xFF0B1220;
    static final int DARK_SOFT = 0xFF1E293B;
    static final int GREEN = 0xFF0F7A3D;
    static final int RED = 0xFFA52A1A;
    static final int WHITE = 0xFFFFFFFF;

    private Ui() {
    }

    static int dp(Context c, float value) {
        return Math.round(value * c.getResources().getDisplayMetrics().density);
    }

    static TextView text(Context c, CharSequence s, float sp, int color, boolean bold) {
        TextView t = new TextView(c);
        t.setText(s);
        t.setTextSize(sp);
        t.setTextColor(color);
        t.setIncludeFontPadding(false);
        if (bold) {
            t.setTypeface(Typeface.DEFAULT_BOLD);
        }
        return t;
    }

    /** Arabic above Latin, the way the web headings and buttons read. */
    static LinearLayout bi(Context c, String latin, String arabic, float sp, int color, boolean bold) {
        LinearLayout box = new LinearLayout(c);
        box.setOrientation(LinearLayout.VERTICAL);
        box.setLayoutDirection(LinearLayout.LAYOUT_DIRECTION_LTR);
        if (arabic != null && !arabic.isEmpty()) {
            TextView ar = text(c, arabic, sp * 0.94f, color, bold);
            // The lines stack, they do not face each other: an Arabic label that hugged the
            // right edge would leave the Latin line alone on the left, which is not how the
            // web reads and looks broken on a phone.
            ar.setGravity(android.view.Gravity.LEFT);
            box.addView(ar);
        }
        if (latin != null && !latin.isEmpty()) {
            box.addView(text(c, latin, sp, color, bold));
        }
        return box;
    }

    /** One line for tight spots: "Delete · حذف". */
    static TextView biLine(Context c, String latin, String arabic, float sp, int color, boolean bold) {
        return text(c, latin + "  ·  " + arabic, sp, color, bold);
    }

    static LinearLayout column(Context c) {
        LinearLayout box = new LinearLayout(c);
        box.setOrientation(LinearLayout.VERTICAL);
        box.setLayoutDirection(LinearLayout.LAYOUT_DIRECTION_LTR);
        return box;
    }

    static LinearLayout row(Context c) {
        LinearLayout box = new LinearLayout(c);
        box.setOrientation(LinearLayout.HORIZONTAL);
        box.setLayoutDirection(LinearLayout.LAYOUT_DIRECTION_LTR);
        box.setGravity(android.view.Gravity.CENTER_VERTICAL);
        return box;
    }

    static GradientDrawable rounded(int color, float radiusDp, Context c) {
        GradientDrawable d = new GradientDrawable();
        d.setColor(color);
        d.setCornerRadius(dp(c, radiusDp));
        return d;
    }

    static GradientDrawable outlined(int color, float radiusDp, int strokeColor, Context c) {
        GradientDrawable d = rounded(color, radiusDp, c);
        d.setStroke(Math.max(1, dp(c, 1)), strokeColor);
        return d;
    }

    /** A white card with the app's corner radius, ready to be filled and added. */
    static LinearLayout card(Context c) {
        LinearLayout card = column(c);
        card.setBackground(rounded(CARD, 16, c));
        int p = dp(c, 16);
        card.setPadding(p, p, p, p);
        card.setElevation(dp(c, 1.5f));
        return card;
    }

    static void pad(View v, int l, int t, int r, int b) {
        v.setPadding(l, t, r, b);
    }

    static View spacer(Context c, int heightDp) {
        View v = new View(c);
        v.setLayoutParams(new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(c, heightDp)));
        return v;
    }

    static LinearLayout.LayoutParams lp(int w, int h) {
        return new LinearLayout.LayoutParams(w, h);
    }

    /** A hairline under a row, the way the web tables separate records. */
    static View hairline(Context c) {
        View v = new View(c);
        LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, Math.max(1, dp(c, 1)));
        v.setLayoutParams(p);
        v.setBackgroundColor(LINE);
        return v;
    }

    static String money(double value, String currency) {
        return String.format(java.util.Locale.US, "%,.2f %s", value, currency);
    }

    static String qty(double value) {
        return value == Math.rint(value)
                ? String.format(java.util.Locale.US, "%,.0f", value)
                : String.format(java.util.Locale.US, "%,.2f", value);
    }
}
