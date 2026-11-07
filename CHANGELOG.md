# Changelog

All notable changes to S2C MAROC. The version follows the crate version in
`Cargo.toml` and is shown at the bottom of the sidebar.

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
