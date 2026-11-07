# Changelog

All notable changes to S2C MAROC. The version follows the crate version in
`Cargo.toml` and is shown at the bottom of the sidebar.

## 0.7.13 — 2026-11-07

The delete dialog got the room it needed.

### Changed

- The confirmation card is 560 px wide instead of 430, with a 28 px pad, a 50 px trash
  badge, a 20 px title, 14 px body text and roomier buttons, so the question and the
  record it names read at a glance.

## 0.7.12 — 2026-11-07

Deletes ask in the app, not in the browser.

### Changed

- Every delete opens a modal instead of the browser's `confirm()` box: a `<dialog>`
  centred over a dimmed page, asking the question in both languages, naming the record
  (`Acme SARL · CUS-1`), with **Cancel** focused first and Escape to close. Only
  **Delete** posts. `views::confirm_dialog()` draws it once in the shell, `app.js`
  opens it from `data-confirm` on the form, and `views::icon_post_of()` fills in the
  record it would remove.
- The dialog answers through its own buttons, not the `close` event, which some
  Chromium builds never fire; the double-submit guard no longer disables a button whose
  submit was held back for confirmation.

## 0.7.11 — 2026-11-07

The table actions became buttons.

### Changed

- Row actions in every list are 34 px circles — a pale tinted disc with a ring and
  the glyph in the deep brand ink (8.4:1, over WCAG AA), red for delete, inverting to
  a solid disc with a white glyph on hover or focus. They used to be a bare 15 px
  glyph in a transparent 30 px circle, which read as decoration rather than
  something to click. The disc scales with the icon size setting.

### Fixed

- The trash glyph was missing from `src/icons.rs`: the Papicons generator removed the
  whitespace between a tag and its first attribute, so `<path d=…>` shipped as
  `<pathd=…>` and the delete buttons in every list drew an empty circle. The
  generator now folds a line break to a space and only closes `> <` up, and a test
  checks every shipped glyph still carries its path data.

## 0.7.10 — 2026-11-07

Icon size is a setting.

### Added

- **Settings › Icon size**, with the same `S` / `M` / `L` / `XL` steps as the two
  font sizes. It is one multiplier (`--ico`) for every glyph the app draws — the
  sidebar, the panel and sign-out buttons, the settings gear and the round row
  actions — so the frames keep the proportion they were designed at instead of one
  part growing alone. The choice is a cookie (`si_ico`) set by `/icon/{size}` and
  read into `Ctx::ico`, which the shell writes onto `<html data-ico="…">`.

## 0.7.9 — 2026-11-07

The icon set is Papicons.

### Changed

- **Every icon now comes from [Papicons](https://github.com/PapillonApp/Papicons)**
  (MIT), in place of the hand-drawn stroke shapes: filled glyphs on a 24 px grid,
  coloured `currentColor`, so the sidebar, the toolbar buttons, the row-action circles
  and the settings gear all share one family. `src/icons.rs` holds the glyphs and maps
  the app's names to them (`po` is a paper, `receipt` a checklist, `wallet` a card),
  so `views::icon()` stays the only caller and pages never name the icon set.
- A filled glyph reads heavier than a 1.6 px line at the same size, so the frames
  keep their box and nothing about the layout moved.

### Added

- `docs/THIRD-PARTY.md` with the Papicons MIT notice, and `tools/fetch_papicons.py`,
  which regenerates `src/icons.rs` from the set.

## 0.7.8 — 2026-11-07

Every table in a page opens with two header rows.

### Changed

- **The tools are the table's first header row.** The search box and the `+ New`
  button no longer float above the card: they sit in a header cell spanning every
  column, centred, in the same card that holds the column names — `views::list_table()`
  and `views::list_table_actions()`. The second header row still names the columns.
- The credit workspace, the event log and the broker queues had no search box; they
  have one now in the same place, with no `+ New` because none of the three can create
  a record. They filter the rows already in hand with `pages::row_matches()`.

### Fixed

- A search that matches nothing now keeps the table's two header rows and shows
  "No records yet" in the body, so the box that produced the empty result can be
  seen and cleared instead of disappearing with the table.

## 0.7.7 — 2026-11-07

Every list heads its table with the same row.

### Added

- **A search box on every list**, not only on products, suppliers and customers:
  purchase orders, goods receipts, supplier invoices, quotes, customer orders,
  delivery notes, customer invoices, payments and tickets now filter by their
  number and the party on the document. `pages::list_filter()` builds that Mongo
  filter, so the term is matched as literal text (`PO (12)` finds `PO (12)`)
  and the box keeps what was typed.

### Changed

- The row above a table — search box, then `+ New` — sits in the middle of the row
  instead of the right edge, and both parts come from `pages::search_form()`, so the
  box, its placeholder and its button are the same on every page.

## 0.7.6 — 2026-11-07

One footer, on every page.

### Changed

- **The page bar** is one fragment (`views::foot`) now: brand dot, the app's two
  names and the version pill at the left, the `© <year> (koul)` credit at the far
  end. The app shell, the sign-in page and the error pages all insert it, so the
  three of them can no longer disagree.

### Fixed

- The sign-in page and the error pages no longer carry a footer of their own — the
  sign-in bar was a 40 / 20 / 40 grid of its own, and an error page had none at
  all. They now end on the same bar as every list and form.
- An error page centres its card in the space the bar leaves, instead of sitting
  high because the bar was also claiming an auto margin.

## 0.7.5 — 2026-11-07

The group headers keep a name whole.

### Fixed

- A group whose Latin name did not fit beside the Arabic half was broken at one of its
  hyphens (`PROCURE-TO-` / `PAY`); it now moves whole onto a second line, while a short
  group name (`In stock`, `Administration`) stays on one line with its Arabic half.
- The fold chevron sits outside the flow, so the label keeps the full width of the row.

## 0.7.4 — 2026-11-07

The sidebar groups fold.

### Added

- **Procure-to-Pay**, **Quote-to-Cash**, **In Stock** and **Administration** are now
  collapsible: clicking a group header folds its links away, with a chevron that
  turns as it opens. Each group is a `<details>`, so folding works without
  JavaScript; `app.js` only remembers the folded groups in `localStorage`
  (`s2c-nav-secs`).
- The group holding the current page is always reopened, so the active link is never
  hidden, and the icon rail keeps every link visible since it hides the headers.

## 0.7.3 — 2026-11-07

Row actions in tables are icons only.

### Changed

- Every action in a list table's last column is now an **icon-only circle** — open,
  receive, PDF, record payment, deliver, invoice, print, event log and delete — so the
  column keeps one width whatever the duo. The bilingual label moves to `title` and
  `aria-label`, so hovering or a screen reader still reads `Open · فتح`.
- `views::icon_link()` and `views::icon_post()` build them; a circle tints brand blue
  on hover and only the delete turns red.
- The pill rule for labelled action buttons is gone with the labels.

## 0.7.2 — 2026-11-07

Table headers get the spacing buttons already had.

### Changed

- A header cell's stacked pair (Arabic over EN/FR) sat with its two lines touching;
  it now keeps **3 px** between them, matching buttons, while staying centred in the
  cell.

## 0.7.1 — 2026-11-07

Buttons centre their bilingual label.

### Changed

- A button's stacked pair was aligned to the top-left of its box; it is now
  **centred horizontally and vertically** in the button, whatever the length of the
  two halves, and the lines sit **3 px** apart instead of touching.
- Table headers keep their own spacing and left-aligned second line, so only
  buttons changed.

## 0.7.0 — 2026-11-07

The sign-in page is framed by a fixed header and footer.

### Added

- **A fixed header on `/login`** in the same 40 / 20 / 40 columns as the app bar:
  the `S2C` mark and the Latin app name, the `Settings` menu in the middle, and the
  Arabic app name at the far end. It stays on screen while the form scrolls.
- **A fixed footer** with the copyright (`© <year> (koul)`) at the left, an empty
  middle column, and the version badge at the right — its pill palette adapted to
  the dark sign-in background.

### Changed

- The sign-in card is centred between the two bars instead of the whole viewport,
  and the app name no longer repeats above the form: the hero keeps the `S2C` mark
  and the two taglines, the names moved to the header.
- The header keeps its 40 / 20 / 40 split on a phone, with tighter type and padding,
  instead of stacking.

## 0.6.1 — 2026-11-07

Table headers are centred and the actions column holds round, centred buttons.

### Changed

- **Every table header is centred**, both axes, in all list and document tables. The
  stacked bilingual halves centre against each other, so the Arabic half no longer
  hangs left under a wide Latin label. Column headers stay centred for numeric
  columns too; the values underneath keep their right alignment.
- **The actions column is a column of round buttons in the middle of the cell.**
  `views::table_actions()` marks its last cell, so only lists that really end with
  an actions column are affected; labelled actions become pills and icon-only
  buttons — including the remove-line button on a document editor — become circles.
  Headers and the actions column follow the same rule in RTL.
- **`views::table_actions()`** joins `views::table()`; both share one renderer, and
  the plain table keeps plain cells for display-only lists.

## 0.6.0 — 2026-11-06

Labels are arranged by element type, and text size gained a fourth step.

### Changed

- **One layout rule per element type.** `tr()` now returns the bilingual pair as
  markup (`<span class="bi">` holding `.lat` and `.ar`), so CSS decides how the two
  languages sit: **buttons and table headers stack them with Arabic on top and
  EN/FR underneath**, while labels, field hints, cells, cards, titles, badges and
  the sidebar keep them **in one line, English at the left and Arabic at the right
  end**. All pages follow the same rule, because the arrangement lives in the
  stylesheet rather than in each template.
- **`tr_text()`** returns the same pair as plain `English · Arabic` for places that
  cannot hold markup: attributes, `title` tooltips, `<title>` and the PDF writer.
  The `.sep` span that separates the halves is hidden by CSS, so `<option>` text and
  copy/paste read naturally too.
- **Font size has four steps per face: `S` 100 %, `M` 150 % (default), `L` 200 %,
  `XL` 250 %.** They replace the previous 90/114 % and 120/160 % scales and apply
  the same ratio to Comfortaa and VIP Rawy, so the duo keeps its balance at every
  size. `si_fs_lat` / `si_fs_ar` accept `xl` and `/font/{face}/{size}` serves it.

### Fixed

- Search boxes, line-item descriptions and the panel button no longer print the
  label markup inside their `placeholder` / `title` attributes — attributes now use
  the plain `tr_text()` string.
- Ageing rows on the dashboard, the empty state and the form action buttons showed
  escaped markup (`&lt;span class="bi"`) after the labels became markup; they are
  inserted as HTML again.
- Line-item table headers align from the top, so the stacked halves stay level when
  a column is narrow at the larger text steps.
- `table()` documents and tests its contract: headers and cells arrive as markup
  built by `tr()` / badges, and every caller escapes its data.

## 0.5.1 — 2026-11-06

The sidebar menu collapses.

### Added

- A panel button at the top of the sidebar folds the whole menu down to a 78 px
  icon rail and opens it again; the page content takes the freed width.
- Nav items keep their bilingual label as a `title`, so the collapsed icons are
  still identifiable on hover.
- The state lives in `localStorage` (`s2c-nav`) and a small inline script in
  `<head>` applies it before the first paint, so a collapsed menu does not flash
  open on load. `aria-expanded` follows the state, and the toggle is hidden on
  phones where the sidebar is a stacked block.

## 0.5.0 — 2026-11-06

Every screen is bilingual and the header became a three-column bar.

### Changed

- **Both languages on every page.** A duo no longer picks one language for the
  screen: with `AR / EN` selected, every label — titles, sidebar, tables, forms,
  buttons, badges, flash messages, event log — shows English *and* Arabic, e.g.
  `Suppliers · الموردون`. The same with `AR / FR` and French.
- The Latin half is Comfortaa and the Arabic half VIP Rawy inside the same element
  (the font stack falls through per glyph), so a bilingual label needs no extra
  markup and keeps its own face on each side.
- Bilingual pages always read left to right: `Lang::dir()` is `ltr` for every duo,
  so the RTL layout is no longer used.
- The header is a 40 % / 20 % / 40 % grid: page title in the duo's Latin language
  on the left, sign-out and `Settings` in the middle, the Arabic title on the right
  (all pages, including the sign-in screen).
- Each duo is now one row in `Settings › Languages` (`AR / EN`, `AR / FR`), marked
  with a tick when selected, instead of a code-by-code switcher.
- Signing in no longer overwrites the language cookie, so the chosen duo survives
  the login round-trip.

### Added

- `Settings › Font size`, one submenu per font — `Comfortaa` and `VIP Rawy`, each
  with `S` / `M` / `L` — so the two halves of a label can be sized independently.
  Implemented with `size-adjust` font faces (`Comfortaa S/L`, `VIP Rawy S/L`, the
  Arabic ones limited to the Arabic Unicode blocks) and the `si_fs_lat` /
  `si_fs_ar` cookies; routes `/font/{face}/{size}`.
- `views::Title` carries the page title in both languages so the header can show
  each half in its own column.
- Settings submenus are `<details>` elements: they work without JavaScript and the
  menu closes on an outside click or `Escape`.

## 0.4.3 — 2026-11-06

The language switcher is now a single header menu with two duos.

### Changed

- The three separate language links in the sidebar footer (and under the login
  card) became one **Languages** dropdown centred in the top bar, labelled in the
  current language, with the same menu on the sign-in screen.
- The menu lists exactly two duos — `AR / EN` and `AR / FR` — instead of three
  standalone languages. Each code is a link and the current language is
  highlighted in both rows.
- Fonts follow the language: `EN`/`FR` render in **Comfortaa**, `AR` in
  **VIP Rawy**. The codes are styled per language inside the menu, so they keep
  their own face even when the surrounding page is in the other language.

### Added

- `lbl_languages` (Languages / Langues / اللغات) and a globe icon for the trigger.
- The menu closes on an outside click or `Escape` (it is a `<details>` element, so
  it also works without JavaScript).

## 0.4.2 — 2026-11-06

Numbers in Arabic now wear the same face as the rest of the app.

### Changed

- The Arabic UI keeps VIP Rawy for its letters but draws every digit, decimal
  point, thousands comma and percent sign with **Comfortaa**: a digits-only
  `Comfortaa Numbers` family (300/400/700, declared with `unicode-range`) leads the
  RTL stack, and the numeric stack used by `.num` cells, totals, aging rows, the
  version pill and the dashboard cards is Comfortaa in RTL too.

## 0.4.1 — 2026-11-06

Finding the app is now documented instead of guessed.

### Added

- The startup log states the browsable URL:
  `listening — open the url in a browser local=0.0.0.0:8080 url=http://localhost:8080`.
- README: a **Where is the app?** table (host binary vs container, and why Docker
  Desktop shows no port for the app), plus a **RabbitMQ log noise** section that
  explains the boot, shutdown and `client unexpectedly closed TCP connection` lines.

### Changed

- The compose app service publishes `${APP_PORT:-8080}:8080`, so a second install
  can avoid a port clash with `APP_PORT=8090 docker compose up`.
- `.env.example` seeds `COMPANY_NAME="S2C MAROC"`.

## 0.4.0 — 2026-11-05

The app is now **S2C MAROC**, and it trades in dirhams.

### Changed

- **Name** — the product name, window titles, footer, brand mark (`S2C`), seeded
  company (`COMPANY_NAME`, default `S2C MAROC`), liveness text, README and API
  docs all say S2C MAROC. The crate and binary keep the historical identifier
  `stockinvoice`, so existing run scripts and log filters still work.
- **Currency** — a fresh database seeds its company in **MAD** instead of USD.
  The code is free text on the company form, so a company can still be switched
  to EUR, USD or anything else.

## 0.3.1 — 2026-11-05

Header and footer get the theme, and stop getting in the way on a small screen.

### Changed

- **Top bar** — aurora tint behind the glass blur, a gradient hairline along its
  bottom edge, a gradient rule before the page title, and a soft shadow so
  scrolling cards pass under it cleanly.
- **Footer** — a real bar instead of a stray line of text: gradient brand dot, the
  app version as a pill, the copyright year and the company name, separated from
  the content by a hairline and pinned to the bottom of the viewport.

### Fixed

- On a narrow screen the whole navigation was rendered *above* the header, so the
  page title, the signed-in user and the sign-out button were pushed below a menu
  taller than the viewport. The order is now header, navigation, content, footer.

## 0.3.0 — 2026-11-05

A new visual identity, and the branding fonts now actually load.

### Changed

- **Theme** — rewritten stylesheet: aurora gradient canvas and sidebar, glass top
  bar, gradient stat cards with accent rules, card-wrapped tables, pill badges
  with status dots, gradient primary buttons and a dark, grid-lit sign-in screen.
  RTL rules, print output and the responsive breakpoints were carried over.
- **Typography** — Comfortaa (300/400/700) drives the Latin UI and VIP Rawy the
  Arabic one, both declared with `@font-face` at their real weights.
- Page headers are now just the action toolbar: the top bar already names the
  page (or the document number), so pages no longer print a second, larger title
  above a card that repeats the same section name.

### Added

- `/static/fonts/{file}` serves the bundled faces (`font/ttf`, one-year cache);
  it only accepts font extensions and rejects path traversal.
- Stylesheet and script URLs carry the crate version *and* a checksum of their
  contents (`?v=0.3.0-92cbc54f`), so a rebuild is never served from an hour-old
  browser cache — the version alone was not enough while iterating on the theme.

### Fixed

- The theme referenced `VIPRawy-Regular.ttf`, but the shipped file is
  `VIP-Rawy-Regular.ttf`, and no route served `/static/fonts/*` at all — so both
  branding fonts silently fell back to system faces.

## 0.2.0 — 2026-11-05

Arabic documents are now real documents rather than English fallbacks, and the
sign-in page is served as HTML.

### Added

- **Arabic PDFs** — the writer embeds a Unicode face (`static/fonts/arabic.ttf`,
  Amiri, OFL) as a Type0 / Identity-H font and shapes the text itself:
  contextual isolated/final/initial/medial forms, lam-alef ligatures, combining
  marks and bidi reordering, so Latin codes and amounts keep their direction.
- **`src/fonts.rs`** — a small TrueType reader (`cmap` formats 4 and 12, `hmtx`,
  `head`, `hhea`, `maxp`) plus the shaping/bidi engine, with unit tests for
  joining, ligatures, marks and run ordering.
- **`tools/gen_arabic_forms.py`** — regenerates the presentation-form and joining
  table from the Unicode character database instead of hand-written data.
- **`ToUnicode` CMap** — Arabic text in a generated PDF is selectable, copyable
  and searchable, and copies as normal Arabic letters.
- Arabic receipts (`/tickets/{id}/pdf`) print in Arabic too.

### Changed

- The embedded face is used for any string WinAnsi cannot encode, so an English
  or French invoice carrying an Arabic customer name or address prints it
  instead of `?????`. It is embedded only when such a glyph is actually drawn.
- `PDF_ARABIC_FONT` selects a different face (for example a licensed VIP RAWY);
  without the file the previous English/French fallback still applies.
- The sidebar version now comes from `CARGO_PKG_VERSION`.

### Fixed

- The sign-in page returned a bare `String` body without a content type, so
  browsers rendered the markup as text and never loaded the stylesheet.

## 0.1.0 — 2026-11-05

First complete release: the whole Procure-to-Pay and Quote-to-Cash chain runs on
one binary.

### Added

- **Procure-to-Pay** — suppliers, purchase orders (draft → sent), goods receipts
  with stock posting and automatic PO status, supplier invoices
  (draft → issued → paid/cancelled), payments with partial settlement.
- **Quote-to-Cash** — customers, quotes (draft → sent → validated), one-step
  quote validation that creates the sales order *and* the customer invoice,
  customer orders, delivery notes with stock deduction, customer invoices with
  PDF download.
- **Products & stock** — SKU catalogue, stock value, low-stock alerts, movement
  history, manual adjustments with audit trail.
- **Credit** — per-customer limit, live exposure, available credit and 0/30/60/90
  aging buckets; limits enforced on quote validation, order confirmation and
  invoice creation.
- **Direct tickets (POS)** — multi-line sale, stock out, receipt preview, branded
  PDF and raw ESC/POS output with a Web Bluetooth print button.
- **Admin panel** — company profile, invoice template (colours, header, footer,
  terms, JPEG logo), user management, password change, event log, RabbitMQ
  broker monitor.
- **Native PDF invoices** — dependency-free writer with per-company branding,
  payment status and automatic pagination.
- **Multi-language UI** — English, French and Arabic (RTL) with a persistent
  switcher.
- **RabbitMQ events** — topic exchange, 13 routing keys, consumer that mirrors
  messages into MongoDB, graceful degradation when the broker is down.
- **JSON API v1** for the Android client (auth, dashboard, master data,
  documents, tickets, credit) documented in `docs/API.md`.
- **Docker** — multi-stage image plus compose stack with local MongoDB and
  RabbitMQ.
- Unit tests for money/format helpers, HTML rendering and ESC/POS output.
