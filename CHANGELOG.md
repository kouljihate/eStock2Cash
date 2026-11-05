# Changelog

All notable changes to StockInvoice. The version follows the crate version in
`Cargo.toml` and is shown at the bottom of the sidebar.

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
- Stylesheet and script URLs carry the crate version, so a new build is never
  served from a stale browser cache.

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
