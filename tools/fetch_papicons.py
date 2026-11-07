import os
import re
import urllib.request

MAP = {
    'dashboard': 'home',
    'suppliers': 'pillar',
    'po': 'paper',
    'receipt': 'tasks',
    'bill': 'newspaper',
    'wallet': 'card',
    'customers': 'user',
    'quote': 'text-bubble',
    'order': 'list',
    'delivery': 'car',
    'products': 'archive',
    'credit': 'piggy-bank',
    'ticket': 'qr-code',
    'admin': 'gears',
    'events': 'clock',
    'broker': 'link',
    'cog': 'gears',
    'logout': 'logout',
    'panel': 'menu',
    'chevron': 'chevron-down',
    'eye': 'external-link',
    'pencil': 'pen',
    'trash': 'trash',
    'file': 'paper',
    'print': 'newspaper',
    'globe': 'globe',
}
FALLBACK = 'placeholder'
BASE = 'https://cdn.jsdelivr.net/gh/PapillonApp/Papicons@master/icons/{}.svg'
CACHE = '.paplicons_tmp'

os.makedirs(CACHE, exist_ok=True)


def fetch(name):
    path = os.path.join(CACHE, name + '.svg')
    if not os.path.exists(path):
        with urllib.request.urlopen(BASE.format(name), timeout=30) as r:
            data = r.read().decode('utf-8')
        open(path, 'w', encoding='utf-8', newline='\n').write(data)
    return open(path, encoding='utf-8').read()


wanted = sorted(set(MAP.values()) | {FALLBACK})
glyphs = {}
for pap in wanted:
    raw = fetch(pap)
    m = re.match(r'\s*<svg\b([^>]*)>(.*)</svg>\s*$', raw, re.S)
    assert m, pap
    attrs, body = m.group(1), m.group(2)
    vb = re.search(r'viewBox="([^"]+)"', attrs)
    view_box = vb.group(1) if vb else '0 0 24 24'
    body = body.strip()
    # One line per glyph: a break between two tags is noise and closes up, but a
    # break between two attributes is the space that keeps them apart (`<path d=`
    # must not become `<pathd=`), so it folds to a single space.
    body = re.sub(r'>\s+<', '><', body)
    body = re.sub(r'\s*\n\s*', ' ', body)
    body = re.sub(r'\s{2,}', ' ', body)
    # the set is drawn in black; the frame's fill is what colours it here
    body = body.replace('fill="black"', 'fill="currentColor"')
    body = body.replace('stroke="black"', 'stroke="currentColor"')
    body = re.sub(r'fill="#[0-9a-fA-F]{3,8}"', 'fill="currentColor"', body)
    body = re.sub(r'stroke="#[0-9a-fA-F]{3,8}"', 'stroke="currentColor"', body)
    body = body.replace('fill="white"', 'fill="currentColor"')
    assert '"#' not in body, pap
    glyphs[pap] = (view_box, body)

lines = ['//! Icons — Papicons, the open set drawn by Tom Things for Papillon.',
         '//!',
         '//! <https://github.com/PapillonApp/Papicons>, MIT (see `docs/THIRD-PARTY.md`).',
         '//! Each glyph below is the inner markup of one of its SVGs, with the black it was',
         '//! drawn in swapped for `currentColor`, so an icon takes the colour of the text, link',
         '//! or button that carries it. The set is drawn on a 24 px grid; `svg()` supplies the',
         '//! frame and `app.css` supplies the size.',
         '',
         '/// The inner markup and grid of one glyph. The app keeps its own names — `po` is a',
         '/// paper, `receipt` a checklist, `wallet` a card — mapped here to the Papicons file',
         '/// that carries the same meaning, so a call site never names the icon set.',
         'fn glyph(name: &str) -> (&\'static str, &\'static str) {',
         '    match name {']
for app_name, pap in sorted(MAP.items()):
    view_box, body = glyphs[pap]
    lines.append('        "%s" => ("%s", r#"%s"#),' % (app_name, view_box, body))
view_box, body = glyphs[FALLBACK]
lines.append('        _ => ("%s", r#"%s"#),' % (view_box, body))
lines += ['    }', '}', '']
lines += ['/// One icon, ready to drop into a button, a link or a table cell: a filled',
          '/// `currentColor` glyph in a frame that scales with whatever holds it.',
          'pub fn svg(name: &str) -> String {',
          '    let (view_box, body) = glyph(name);',
          '    format!("<svg viewBox=\\"{view_box}\\" fill=\\"currentColor\\" aria-hidden=\\"true\\">{body}</svg>")',
          '}', '']
open('src/icons.rs', 'w', encoding='utf-8', newline='\r\n').write('\n'.join(lines))
print('wrote src/icons.rs with', len(MAP), 'names')
