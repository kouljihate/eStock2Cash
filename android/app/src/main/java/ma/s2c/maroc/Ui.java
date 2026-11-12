package ma.s2c.maroc;

import android.content.Context;
import android.content.res.AssetManager;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.text.SpannableString;
import android.text.Spanned;
import android.text.TextPaint;
import android.text.style.CharacterStyle;
import android.text.style.UpdateAppearance;
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

    /* ------------------------------------------------------------------- fonts */

    /*
     * The same two faces the web app uses: Comfortaa for Latin, VIP Rawy for Arabic, with a
     * bold cut of each. A phone cannot pick a font by script the way CSS does with a font
     * stack, so every string is cut into runs of Arabic and non-Arabic characters and each
     * run is given its own face. That is what keeps "Delete · حذف" looking like the browser
     * instead of letting one font win the whole line — and it is why digits inside Arabic
     * labels stay in Comfortaa, exactly like the web's numerals-only faces.
     */

    private static Typeface latRegular;
    private static Typeface latBold;
    private static Typeface arRegular;
    private static Typeface arBold;

    private static synchronized void loadFonts(Context c) {
        if (latRegular != null && arRegular != null) {
            return;
        }
        AssetManager assets = c.getApplicationContext().getAssets();
        latRegular = face(assets, "fonts/Comfortaa-Regular.ttf", Typeface.DEFAULT);
        latBold = face(assets, "fonts/Comfortaa-Bold.ttf", Typeface.DEFAULT_BOLD);
        arRegular = face(assets, "fonts/VIP-Rawy-Regular.ttf", latRegular);
        arBold = face(assets, "fonts/VIP-Rawy-Bold.ttf", latBold);
    }

    private static Typeface face(AssetManager assets, String path, Typeface fallback) {
        try {
            return Typeface.createFromAsset(assets, path);
        } catch (Exception e) {
            // A font missing from the package must not take the screen down with it: the
            // system face is a duller app, not a blank one.
            return fallback;
        }
    }

    /** The Latin face; used for inputs, where spans would not survive typing. */
    static Typeface latin(Context c, boolean bold) {
        loadFonts(c);
        return bold ? latBold : latRegular;
    }

    static Typeface arabic(Context c, boolean bold) {
        loadFonts(c);
        return bold ? arBold : arRegular;
    }

    private static boolean isArabic(char ch) {
        return (ch >= 0x0600 && ch <= 0x06FF)      // Arabic
                || (ch >= 0x0750 && ch <= 0x077F)  // Arabic Supplement
                || (ch >= 0x08A0 && ch <= 0x08FF)  // Arabic Extended-A
                || (ch >= 0xFB50 && ch <= 0xFDFF)  // Presentation forms A
                || (ch >= 0xFE70 && ch <= 0xFEFF); // Presentation forms B
    }

    /** The string, with each run of Arabic or Latin letters set in its own face. */
    static CharSequence font(Context c, CharSequence s, boolean bold) {
        if (s == null || s.length() == 0) {
            return s;
        }
        loadFonts(c);
        SpannableString out = new SpannableString(s);
        int n = s.length();
        int i = 0;
        while (i < n) {
            boolean arabic = isArabic(s.charAt(i));
            int j = i + 1;
            while (j < n && isArabic(s.charAt(j)) == arabic) {
                j++;
            }
            Typeface face = arabic ? (bold ? arBold : arRegular) : (bold ? latBold : latRegular);
            out.setSpan(new Face(face), i, j, Spanned.SPAN_INCLUSIVE_EXCLUSIVE);
            i = j;
        }
        return out;
    }

    /** One typeface applied to one stretch of a string. */
    private static final class Face extends CharacterStyle implements UpdateAppearance {
        private final Typeface typeface;

        Face(Typeface typeface) {
            this.typeface = typeface;
        }

        @Override
        public void updateDrawState(TextPaint paint) {
            paint.setTypeface(typeface);
        }
    }

    static int dp(Context c, float value) {
        return Math.round(value * c.getResources().getDisplayMetrics().density);
    }

    static TextView text(Context c, CharSequence s, float sp, int color, boolean bold) {
        TextView t = new TextView(c);
        t.setText(font(c, s, bold));
        t.setTextSize(sp);
        t.setTextColor(color);
        t.setIncludeFontPadding(false);
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
