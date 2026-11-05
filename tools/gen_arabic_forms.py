#!/usr/bin/env python3
"""Generate src/arabic_forms.rs from the Unicode character database.

Arabic has no shaping information of its own in a PDF: a producer has to pick
the isolated/final/initial/medial glyph variant itself. Rather than hand-typing
that table (and getting one of the ~40 letters wrong), derive it from
UnicodeData.txt: every presentation form is named
"ARABIC LETTER BEH INITIAL FORM", so the base letter and the joining class fall
out of the names. Letters with initial/medial forms are dual-joining, letters
with only final/isolated are right-joining.

Usage:  python tools/gen_arabic_forms.py [path-to-UnicodeData.txt]
        (downloads the file into target/ucd/ when no path is given)
"""

import os
import re
import sys
import urllib.request

URL = "https://www.unicode.org/Public/UCD/latest/ucd/UnicodeData.txt"
CACHE = os.path.join("target", "ucd", "UnicodeData.txt")

FORM_RE = re.compile(r"^ARABIC LETTER (.+) (ISOLATED|FINAL|INITIAL|MEDIAL) FORM$")
LIG_RE = re.compile(
    r"^ARABIC LIGATURE LAM WITH ALEF(?: WITH (MADDA ABOVE|HAMZA ABOVE|HAMZA BELOW))? "
    r"(ISOLATED|FINAL) FORM$"
)
ALEFS = {
    None: 0x0627,
    "MADDA ABOVE": 0x0622,
    "HAMZA ABOVE": 0x0623,
    "HAMZA BELOW": 0x0625,
}


def load(path):
    if path is None:
        if not os.path.exists(CACHE):
            os.makedirs(os.path.dirname(CACHE), exist_ok=True)
            print(f"downloading {URL}")
            urllib.request.urlretrieve(URL, CACHE)
        path = CACHE
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            fields = line.split(";")
            if len(fields) > 1:
                yield int(fields[0], 16), fields[1]


def main():
    names = dict((name, cp) for cp, name in load(sys.argv[1] if len(sys.argv) > 1 else None))

    forms = {}
    for name, cp in names.items():
        m = FORM_RE.match(name)
        if not m:
            continue
        base = "ARABIC LETTER " + m.group(1)
        if base not in names:
            continue
        forms.setdefault(base, {})[m.group(2)] = cp

    rows = []
    for base, f in forms.items():
        iso, fin = f.get("ISOLATED"), f.get("FINAL")
        ini, med = f.get("INITIAL"), f.get("MEDIAL")
        if iso is None:
            continue
        # A letter that never joins forward has no initial or medial form.
        if (ini is None) != (med is None):
            raise SystemExit(f"inconsistent forms for {base}")
        rows.append((names[base], iso, fin or 0, ini or 0, med or 0))
    rows.sort()
    # 0628 BEH is the canonical four-form letter; guard the derivation.
    beh = [r for r in rows if r[0] == 0x0628][0]
    assert beh[1:] == (0xFE8F, 0xFE90, 0xFE91, 0xFE92), beh

    ligs = []
    for name, cp in names.items():
        m = LIG_RE.match(name)
        if not m:
            continue
        alef = ALEFS[m.group(1)]
        ligs.append((alef, m.group(2) == "FINAL", cp))
    ligs.sort()

    out = [
        "//! Arabic presentation forms, generated from UnicodeData.txt by",
        "//! `tools/gen_arabic_forms.py`. Do not edit by hand.",
        "//!",
        "//! `(base, isolated, final, initial, medial)` - a zero width entry means the",
        "//! letter does not join on that side. Sorted by `base` for binary search.",
        "",
        "#![allow(clippy::unreadable_literal)]",
        "",
        "pub static FORMS: &[(u32, u32, u32, u32, u32)] = &[",
    ]
    for base, iso, fin, ini, med in rows:
        out.append(f"    (0x{base:04X}, 0x{iso:04X}, 0x{fin:04X}, 0x{ini:04X}, 0x{med:04X}),")
    out.append("];")
    out.append("")
    out.append("/// Lam-alef ligatures: `(alef, final?, glyph)`. The isolated form is used")
    out.append("/// when the lam does not join to a preceding letter.")
    out.append("pub static LAM_ALEF: &[(u32, bool, u32)] = &[")
    for alef, final, cp in ligs:
        out.append(f"    (0x{alef:04X}, {str(final).lower()}, 0x{cp:04X}),")
    out.append("];")
    out.append("")

    with open(os.path.join("src", "arabic_forms.rs"), "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join(out))
    print(f"wrote src/arabic_forms.rs: {len(rows)} letters, {len(ligs)} ligatures")


if __name__ == "__main__":
    main()
