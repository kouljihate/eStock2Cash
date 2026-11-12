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
 * The look of the phone app, taken from the web UI and sharpened for a 6-inch screen: the
 * deep navy bar, the indigo action, white cards on a cool page.
 *
 * Two rules are carried over deliberately. The app is always left-to-right, and a bilingual
 * label reads English on the left, Arabic on the right — except a button, where the Arabic
 * sits on top and the English underneath, because a button is read as a whole rather than
 * skimmed left to right.
 */
final class Ui {

    static final int BG = 0xFFF3F5FB;
    static final int CARD = 0xFFFFFFFF;
    static final int INK = 0xFF0E1729;
    static final int MUTED = 0xFF6B7A90;
    static final int LINE = 0xFFE4E9F3;
    static final int BRAND = 0xFF4F46E5;
    static final int BRAND_DARK = 0xFF4338CA;
    static final int BRAND_LIGHT = 0xFF6D66F0;
    static final int BRAND_SOFT = 0xFFEDEDFE;
    static final int DARK = 0xFF0B1220;
    static final int DARK_LIGHT = 0xFF1B2A4A;
    static final int DARK_SOFT = 0xFF1E293B;
    static final int GREEN = 0xFF0E7A43;
    static final int GREEN_SOFT = 0xFFE6F7EE;
    static final int RED = 0xFFB3261E;
    static final int RED_SOFT = 0xFFFDECEB;
    static final int AMBER = 0xFFA15C07;
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

    /**
     * A button's label: Arabic on top, English underneath. Buttons are read as a block, so
     * they are the one place the two languages stack instead of facing each other.
     */
    static CharSequence buttonLabel(Context c, String latin, String arabic) {
        return font(c, arabic + "\n" + latin, true);
    }

    static int dp(Context c, float value) {
        return Math.round(value * c.getResources().getDisplayMetrics().density);
    }

    /* ------------------------------------------------------------------ layout */

    static TextView text(Context c, CharSequence s, float sp, int color, boolean bold) {
        TextView t = new TextView(c);
        // Size first, text last: a layout built before the size is known breaks its lines for
        // one font size and then draws them at another, which runs the words off the edge.
        t.setTextSize(sp);
        t.setTextColor(color);
        t.setIncludeFontPadding(false);
        dressed(c, t, s, bold);
        return t;
    }

    /**
     * A string in one script is set by giving the whole view that typeface — spans are kept
     * for the mixed labels only, because a span-altered string is measured by the platform
     * slightly differently from how it is drawn, and long paragraphs came out ragged.
     */
    private static void dressed(Context c, TextView t, CharSequence s, boolean bold) {
        if (s == null) {
            return;
        }
        loadFonts(c);
        boolean anyArabic = false;
        boolean anyLatin = false;
        for (int i = 0; i < s.length(); i++) {
            char ch = s.charAt(i);
            if (isArabic(ch)) {
                anyArabic = true;
            } else if (Character.isLetter(ch)) {
                anyLatin = true;
            }
            if (anyArabic && anyLatin) {
                break;
            }
        }
        if (anyArabic && anyLatin) {
            t.setText(font(c, s, bold));
            return;
        }
        t.setTypeface(anyArabic ? arabic(c, bold) : latin(c, bold));
        t.setText(s);
    }

    /**
     * English on the left, Arabic on the right — the app's standing rule for anything that
     * is not a button. Both lines share the row, so the Arabic really sits at the edge.
     */
    static LinearLayout bi(Context c, String latin, String arabic, float sp, int color, boolean bold) {
        LinearLayout row = row(c);
        // Full width, so the Arabic has room for a whole phrase instead of being squeezed into
        // a column and left wrapping between two words.
        row.setLayoutParams(new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT));
        boolean hasLatin = latin != null && !latin.isEmpty();
        boolean hasArabic = arabic != null && !arabic.isEmpty();
        if (hasLatin) {
            TextView left = text(c, latin, sp, color, bold);
            left.setMaxLines(2);
            row.addView(left);
        }
        if (hasLatin && hasArabic) {
            // A weight on the English side would starve the Arabic of the room it needs, and
            // the phrase would break between two words. A spacer takes the slack instead.
            View gap = new View(c);
            row.addView(gap, new LinearLayout.LayoutParams(0, 1, 1f));
        }
        if (hasArabic) {
            TextView right = text(c, arabic, sp, color, bold);
            right.setGravity(android.view.Gravity.END);
            right.setMaxLines(2);
            right.setPadding(dp(c, 8), 0, 0, 0);
            row.addView(right);
        }
        return row;
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

    /* ------------------------------------------------------------------ shapes */

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

    /** A diagonal wash, for the hero and for the primary action. */
    static GradientDrawable gradient(int from, int to, float radiusDp, Context c) {
        GradientDrawable d = new GradientDrawable(GradientDrawable.Orientation.TL_BR,
                new int[]{from, to});
        d.setCornerRadius(dp(c, radiusDp));
        return d;
    }

    /** The indigo face of every primary button. */
    static GradientDrawable brand(Context c, float radiusDp) {
        return gradient(BRAND_LIGHT, BRAND_DARK, radiusDp, c);
    }

    /** The gradient wash of a page hero, rounded along its bottom edge only. */
    static GradientDrawable hero(Context c, int from, int to, float radiusDp) {
        GradientDrawable d = new GradientDrawable(GradientDrawable.Orientation.TL_BR, new int[]{from, to});
        float r = dp(c, radiusDp);
        d.setCornerRadii(new float[]{0, 0, 0, 0, r, r, r, r});
        return d;
    }

    /**
     * A white card: rounded, hairline-edged, barely lifted off the page. It fills the width by
     * default — a wrap-content card lets a long sentence measure itself as one endless line
     * and run off the side of the phone.
     */
    static LinearLayout card(Context c) {
        LinearLayout card = column(c);
        card.setBackground(outlined(CARD, 18, LINE, c));
        card.setLayoutParams(new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT));
        int p = dp(c, 16);
        card.setPadding(p, p, p, p);
        card.setElevation(dp(c, 2));
        return card;
    }

    /** A quiet pill for counts and states. */
    static TextView pill(Context c, CharSequence s, int bg, int fg) {
        TextView t = text(c, s, 11, fg, true);
        t.setGravity(android.view.Gravity.CENTER);
        t.setBackground(rounded(bg, 999, c));
        t.setPadding(dp(c, 10), dp(c, 4), dp(c, 10), dp(c, 4));
        return t;
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
