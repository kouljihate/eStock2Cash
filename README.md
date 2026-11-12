# S2C MAROC

Moroccan Procure-to-Pay / Quote-to-Cash. The seeded company is **S2C MAROC** with
**MAD** (dirham) as its currency; both are editable in **Administration → Company**.

A single-binary **Rust** business application for the full **Procure-to-Pay** and
**Quote-to-Cash** lifecycle with MongoDB, role-based access, per-company
branding, multi-language UI (EN / FR / AR) and a JSON API for the Android client.

UI is rendered server-side with **Leptos** (`view!`) on **Axum**; MongoDB stores
all documents; **RabbitMQ** carries domain events that are mirrored back into
MongoDB.

```
Procure-to-Pay   Supplier → Purchase Order → Goods Receipt → Supplier Invoice → Payment
Quote-to-Cash    Quote → Customer Order → Delivery Note → Customer Invoice
Cross-cutting    Products & stock, Credit limits & aging, Direct POS tickets,
                 Admin panel, Invoice PDF, ESC/POS printing, i18n, Events
```

---

## Status

| Module | Description | Status |
|--------|-------------|--------|
| **Suppliers** | Vendor master data, contacts, legal/tax IDs | Ready |
| **Purchase Orders** | Draft → Sent, multi-line, totals, PDF-ready | Ready |
| **Goods Receipts** | Receive against a PO, posting increases stock | Ready |
| **Supplier Invoices** | Bills with issue/cancel workflow and payments | Ready |
| **Payments** | In/out payments, partial settlement, status roll-up | Ready |
| **Customers** | Master data, credit limit, payment terms | Ready |
| **Quotes** | Multi-line quotes; validate creates order **and** invoice | Ready |
| **Customer Orders** | Confirmed orders, credit check, deliver & invoice links | Ready |
| **Delivery Notes** | Posting decreases stock, closes the order | Ready |
| **Customer Invoices** | Status workflow, outstanding balance, branded PDF | Ready |
| **Products** | SKU catalogue, stock, movements, low-stock alerts | Ready |
| **Credit** | Per-customer exposure, available credit, aging buckets | Ready |
| **Direct Tickets** | POS sale, stock out, receipt PDF + ESC/POS + Bluetooth | Ready |
| **Admin** | Company profile, invoice template, users, event log, broker | Ready |
| **PDF invoices** | Native PDF writer with per-company branding | Ready |
| **i18n** | Bilingual Arabic + English/French on every screen, chosen in `Settings` | Ready |
| **RabbitMQ** | Topic events, MongoDB mirror, management monitor | Ready |
| **Docker** | App + local MongoDB + local RabbitMQ | Ready |
| **Android app** | Native client (Java, no WebView): sign in, dashboard, every list | Ready |

---

## Quick start

### Docker (app + MongoDB + RabbitMQ, all local)

```bash
docker compose up --build
# app        http://localhost:8080
# rabbitmq   http://localhost:15672  (stockinvoice / stockinvoice)
```

`APP_PORT` changes the published port: `APP_PORT=8090 docker compose up --build`
serves the app on <http://localhost:8090>.

### Where is the app?

| How it runs | Browse | Docker Desktop |
|-------------|--------|----------------|
| `cargo run` / `./target/debug/stockinvoice.exe` (host binary) | <http://localhost:8080> | *no app container* — Docker only runs MongoDB and RabbitMQ |
| `docker compose up --build` (container `stockinvoice-app`) | <http://localhost:8080> | port `8080:8080` is listed under **Ports** |

The app always prints the URL at startup:
`listening — open the url in a browser local=0.0.0.0:8080 url=http://localhost:8080`.

**No port column for the app in Docker Desktop?** Two causes:

1. Only the dependencies are containerised — `docker compose up -d mongo rabbitmq`
   plus a host binary is the normal dev setup, so no app container exists to show a port.
2. The port is already taken. If the host binary still holds 8080, the app
   container fails with `port is already allocated` and shows no published port.
   Stop the binary (`taskkill //PID <pid> //F` on Windows) or use `APP_PORT`.

The first boot seeds one company and one local administrator:

```
username: admin
password: looklook      # change it in Admin → Change password, or set ADMIN_PASSWORD
```

### Local development

```bash
cp .env.example .env        # then edit (values with spaces must be quoted)
docker compose up -d mongo rabbitmq
cargo run
```

Requirements: Rust 1.85+ (edition 2021). No C toolchain, OpenSSL or Python is
needed — every dependency is pure Rust.

---

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `8080` | HTTP port (`0` falls back to 8080) |
| `SECRET_KEY` | dev value | Session key, 32+ random chars (ephemeral if too short) |
| `JWT_SECRET_KEY` | dev value | JWT signing key, 32+ random chars |
| `JWT_ACCESS_TOKEN_EXPIRES` | `3600` | Token lifetime (seconds) |
| `COOKIE_SECURE` | `0` | Set `1` when serving over HTTPS |
| `MONGO_URI` | `mongodb://localhost:27017/?directConnection=true` | MongoDB URI |
| `MONGO_DB` | `stockinvoice` | Database name |
| `RABBITMQ_URL` | *(unset)* | AMQP endpoint; unset = messaging disabled (app still works) |
| `RABBITMQ_MGMT_URL` | derived | Overrides the management API URL used by the broker monitor |
| `ADMIN_USERNAME` | `admin` | Seeded administrator (empty database only) |
| `ADMIN_PASSWORD` | `looklook` | Seeded administrator password |
| `COMPANY_NAME` | `S2C MAROC` | Seeded company name (rename it in **Administration**) |
| `DEFAULT_LOCALE` | `en` | `en`, `fr` or `ar` |

The seeded company starts in **MAD**; the currency code is free text on the
company form, so it can be switched to EUR, USD or anything else per company.

### Where the data lives

The database is chosen in the app, not compiled in. **Settings → Database**
(`/settings`, under *Administration* in the sidebar) offers two options:

| Option | Meaning |
|--------|---------|
| **Local — this computer** | MongoDB on the same machine (`127.0.0.1:27017`) |
| **Server — IP address** | A MongoDB on the network: its IP and port, plus a user and password if it needs them |

The choice is written to `app-settings.json` next to the binary and applied at
once: the running app swaps its connection, so the next page reads and writes the
new database with no restart. The connection is tested before anything changes —
an unreachable address leaves the app on the database that works. Another database
has its own users, so switching signs everyone out; an empty one is seeded with
`ADMIN_USERNAME` / `ADMIN_PASSWORD` the first time it is opened.

`MONGO_URI` and `MONGO_DB` only seed `app-settings.json` on the **first run**; after
that the file wins (delete it to pick the environment up again). It is git-ignored,
because it can hold a database server's password.

The broker monitor derives the management API host/port/credentials from
`RABBITMQ_URL` (`amqp://user:pass@host:5672/vhost` → `http://host:15672`).

---

## Architecture

```
src/
  main.rs        bootstrap: env, Mongo, indexes, seed, RabbitMQ consumer, Axum
  config.rs      environment configuration
  state.rs       AppState + typed MongoDB collection handles
  db.rs          connection, indexes, per-company numbering, first-run seed
  auth.rs        Argon2id, JWT, request context (Ctx), RBAC, CSRF middleware
  form.rs        raw form parsing (repeated keys → document lines)
  models.rs      domain models, status enums, totals
  services.rs    business rules: stock posting, settlement, credit, aging
  events.rs      RabbitMQ publisher, Mongo mirror consumer, broker monitor
  pdf.rs         native PDF writer (invoice + receipt)
  fonts.rs       TrueType parsing, Arabic shaping, bidi for the PDF writer
  ticket.rs      ESC/POS byte stream for mini printers
  i18n.rs        EN/FR/AR dictionary
  views.rs       Leptos shell (sidebar, topbar), tables, form components
  pages/         one module per domain (handlers + rendered bodies)
  routes.rs      web routing
  api.rs         /api/v1 JSON API (Bearer JWT) for the Android client
static/          app.css, app.js, favicon, fonts/ (arabic.ttf is bundled)
android/         the phone client (see android/README.md)
```

### Data model

Ids are 24-char hex strings, dates are ISO strings, money is `f64` rounded to
2 decimals. Every collection carries `company_id`; every query filters on it.

Collections: `companies`, `users`, `suppliers`, `customers`, `products`,
`stock_moves`, `purchase_orders`, `goods_receipts`, `supplier_invoices`,
`customer_invoices`, `payments`, `quotes`, `sales_orders`, `delivery_notes`,
`tickets`, `counters`, `event_log`.

Document numbering is per company and sequence-based:
`PO-2026-0001`, `GR-…`, `SI-…`, `QUO-…`, `SO-…`, `DN-…`, `CI-…`, `PAY-…`, `TK-…`.

### Business rules

- **Stock** moves only on posting events: receiving a PO (`+`), posting a
  delivery note (`−`), a direct ticket (`−`) and manual adjustments. Every change
  writes a `stock_moves` row.
- **PO status** is recomputed from all posted receipts (`partially_received` /
  `received`).
- **Invoices** settle through payments: `open` → `partially_paid` → `paid`; an
  unpaid invoice past its due date is displayed as `overdue`.
- **Credit**: `0` limit means unlimited; otherwise exposure (outstanding on open
  invoices) + new amount must stay within the limit — checked when validating a
  quote, confirming an order, creating a customer invoice and selling on credit at
  the till.
- **Credit at the till** is paid at the end of the month: the sale writes an open
  customer invoice due on the last day of the month, takes the stock out with a
  `credit_sale` movement, and the customer settles it on payment day through
  *Record payment*.
- **Quote validation** creates a confirmed sales order *and* an open customer
  invoice in one step, and links both back to the quote.
- **Deleting a purchase order** deletes its goods receipts with it; a posted receipt
  gives its stock back first (one `Reversed` movement per product) instead of erasing
  history. Nothing is deleted while money hangs off it: a purchase order billed on a
  supplier invoice, or an invoice that already has payments, is refused with a red
  notice naming what to delete first.

### Events (RabbitMQ → MongoDB)

Every state change publishes to the `stockinvoice` topic exchange:
`po.created`, `po.sent`, `gr.posted`, `supplier_invoice.created`,
`supplier_invoice.issued`, `payment.recorded`, `quote.created`, `quote.sent`,
`quote.accepted`, `order.confirmed`, `delivery.posted`,
`customer_invoice.issued`, `ticket.created` — from the Cash Registry as from a
direct ticket, with the customer on the payload's document.

A background consumer binds `stockinvoice.events` (`#`) and mirrors each message
into `event_log`, which is the Admin → Event Log page. Publishing never blocks or
fails a request: if the broker is down the app keeps working and events resume
when it returns.

### Security

- Argon2id password hashes (per-user random salt), constant-time comparison.
- JWT (HS256) in an `HttpOnly`, `SameSite=Lax` cookie for the web UI and
  `Authorization: Bearer` for the API.
- Every read/write is scoped by `company_id` from the token (never from the
  request body) — a second company's data is invisible even with a valid token.
- Cross-site form posts are rejected when the `Origin` header disagrees with
  `Host`; all mutating routes require an authenticated session.
- The only profile is **Local-Admin** (full access), as specified. `Claims::can`
  and `Permission` keep the RBAC check in place for future profiles.

---

## Invoicing (PDF)

`GET /customer-invoices/{id}/pdf` and `GET /supplier-invoices/{id}/pdf` return a
generated PDF: coloured header band, company block, bill-to block, line table,
totals, payment status and footer — all driven by **Admin → Invoice template**
(primary/accent colour, header, footer, terms, tax visibility, logo).

- Latin text uses the base-14 Helvetica faces and WinAnsi encoding, so French
  and English documents embed nothing (a few kilobytes) and multi-page invoices
  paginate automatically.
- **Arabic** is drawn with a real Unicode font: the writer embeds
  `static/fonts/arabic.ttf` (Type0 / Identity-H) and does the shaping itself —
  contextual isolated/final/initial/medial forms, the lam-alef ligatures and
  bidi reordering so Latin codes and numbers keep their own direction. Arabic
  labels and Arabic data (customer, product, notes) work in every document, and
  an English invoice carrying an Arabic customer name also renders it instead of
  `?????`. The `ToUnicode` CMap keeps the text selectable and searchable.
- The face is embedded only when it is used: an entirely Latin PDF stays small,
  an Arabic one grows by roughly 210 KB (the full font program). Glyph
  subsetting is not implemented.
- **Logo**: paste a base64 JPEG (`data:image/jpeg;base64,…` or the raw
  `/9j/…` form) into the company profile. PNG logos are ignored — PDF image
  embedding uses `DCTDecode`, which is JPEG-only.
- `tools/gen_arabic_forms.py` regenerates `src/arabic_forms.rs` (the joining
  table) from the Unicode character database.

## Languages

Every screen is **bilingual**: each label carries both languages at once — in the
page titles, sidebar, tables, forms, buttons, badges, flash messages and the event
log. The Latin half is drawn in Comfortaa and the Arabic half in VIP Rawy inside the
same element (the stack falls through per glyph), so the two languages sit together
rather than in two separate screens.

`tr()` emits the pair as markup, `<span class="bi"><span class="lat">…</span>` +
`<span class="ar">…</span></span>`, and one CSS rule decides the arrangement per
element type:

| element | arrangement |
| --- | --- |
| table headers | stacked — **Arabic on top, EN/FR underneath**, the pair centred in the cell with 3 px between the lines |
| buttons | stacked the same way, but the pair is **centred across and down the button** with 3 px of air between the two lines |
| labels, fields, cells, cards, nav, titles, badges | one line — **English at the left, Arabic at the right end** |

The `.sep` span between the halves is hidden by CSS, so contexts without styles —
`<option>` text, copy/paste, `title` tooltips — still read `English · Arabic`.
Attributes, `<title>` and PDFs use `tr_text()`, which returns that plain string.

The pair is chosen from one place, the **Settings** dropdown in the middle of the
header, under **Languages**:

```
AR / EN      English first in Latin, Arabic after
AR / FR      Français first in Latin, Arabic after
```

One row is the whole choice: it sets the duo, highlights itself with a tick, and
every page then renders both languages of that duo. The choice lives in the
`si_lang` cookie and survives sign-in; `DEFAULT_LOCALE` decides the first visit.

### The header

The bar is split in three columns — **40 % English text, 20 % controls, 40 % Arabic
text**:

| column | content |
| --- | --- |
| left (40 %) | page title in the duo's Latin language, plus `Signed in as <user>` |
| middle (20 %) | sign-out button and the `Settings` dropdown |
| right (40 %) | the same title in Arabic, right-aligned |

### The page footer

Every page ends on one bar, built by a single fragment (`views::foot`): the app's
two names behind a brand dot, the version pill, and the credit at the far end —

| part | content |
| --- | --- |
| left | gradient dot, `S2C MAROC` / `S2C المغرب`, `v<version>` |
| right | `© <year> (koul)` |

The app shell, the sign-in page and the error pages all insert that same fragment,
so the bar cannot drift between them. It follows the content on a long page and
sits at the bottom of the viewport on a short one; the sign-in page fixes it to
the bottom of its dark chrome, in the night palette.

### The sign-in page

Before you are signed in, the page keeps a fixed header in the same three columns
as the app bar — **40 % English text, 20 % controls, 40 % Arabic text**:

| column | content |
| --- | --- |
| left (40 %) | the `S2C` mark and the app name in the duo's Latin language |
| middle (20 %) | the `Settings` menu (`/login`) |
| right (40 %) | the app name in Arabic, right-aligned |

The shared page footer is pinned below the card in the night palette, so the frame
never scrolls with the form. The card stays centred between the two bars, and the
header keeps its split on a phone by tightening its type.

### The sidebar

The menu collapses to a 78 px icon rail and expands back with the panel button in
its top-right corner. Icons keep their bilingual label as a tooltip, and the state
is stored in `localStorage` (`s2c-nav`) and re-applied by a small inline script in
`<head>`, so a collapsed menu never flashes open while a page loads. On a phone the
sidebar is a stacked block and the toggle is hidden.

The menu opens with **Dashboard** and, straight under it, the **Cash Registry** — the
till, the one entry that belongs to no group because it is where the day starts. Then
the four groups — Procure-to-Pay, Quote-to-Cash, In Stock and Administration — are
collapsible: each is a `<details>` whose header row folds its links away, so the
folding works without JavaScript. `views::nav_section()` emits them (the header is
the `<summary>`, the links sit in `.nav-links`) and `app.js` only remembers the
choice in `localStorage` (`s2c-nav-secs`); the group holding the current page is
always reopened, and the icon rail keeps every link visible because it hides the
headers.

### Tables

Every column header is centred, horizontally and vertically, and the two languages
of a header stack under each other with the short half centred beneath the long one.
Body cells keep their own alignment — numbers stay right-aligned, text stays
left-aligned — so columns remain easy to scan.

The last column of a list table is the **actions** column, and every action in it is
an **icon-only circle** — open, receive, PDF, pay, deliver, invoice, print, movements
and delete. `views::table_actions()` marks that cell, `views::icon_link()` /
`views::icon_post()` build the buttons, and the stylesheet centres them in the cell.

Each circle is 34 px at the `M` icon size, drawn as a pale tinted disc with a ring
round it and the glyph in the deep brand ink (`#4338ca` on `#eef1fe`, 8.4:1 — well
over WCAG AA), so an action reads as a button instead of a glyph floating in the row;
a delete is the same disc in red (`#a52a1a` on `#fdeeee`). Hovering or tabbing to one
inverts it: solid disc, white glyph, and the ring does the work of the mouse cursor.
The disc scales with the **Icon size** setting, so the glyph keeps the same share of
it at every step.

### Delete confirmation

The card is framed in red: a solid red head carrying the trash mark and the question,
a pale red body holding the sentence and the record's name, and a deeper red foot with
the two answers. Red is the frame, not a wash over the text that has to be read.

A delete never posts on the first click. The form carries `data-confirm` (and
`data-confirm-note` naming the record), `views::confirm_dialog()` draws one `<dialog>`
in the shell, and `app.js` opens it: the question is asked in both languages, the page
behind softens out of focus behind a 7 px blur, the focus moves to **Cancel** and
Escape closes it. Only **Delete** posts
the form. There is no backdrop dismissal, so a stray click cannot wave a deletion
through. Browsers without `<dialog>` fall back to their own confirm box.

The card is 560 px wide at most (the viewport minus its margins on a phone), with a
50 px badge, a 20 px title and a 28 px pad, so the record it names is read at a glance
rather than squinted at.

The dialog answers through its two buttons rather than through the `close` event,
because some Chromium builds never deliver that event — and a confirmation that
silently does nothing is the worst possible outcome for a delete.
The bilingual label is not drawn: it becomes the button's `title` and `aria-label`, so
hovering (or a screen reader) still says `Open · فتح` while the column keeps one width
in every language. `views::table()` leaves the cells plain for tables that only
display data.

### Icons

Every glyph comes from [Papicons](https://github.com/PapillonApp/Papicons), the open
set drawn by Tom Things for Papillon (MIT — see `docs/THIRD-PARTY.md`). They are
filled shapes on a 24 px grid, coloured `currentColor`, so an icon takes the tone of
the text, link or button that carries it and the stylesheet alone decides its size.

`src/icons.rs` holds the inner markup of each glyph and the mapping from the app's own
names (`po`, `receipt`, `wallet`, …) to the Papicons file that carries the same
meaning — `views::icon()` is the only caller, so a page names an idea, not an icon set:

| the app asks for | Papicons draws |
| --- | --- |
| `dashboard` / `suppliers` / `customers` | home / pillar / user |
| `po` / `receipt` / `bill` | paper / tasks / newspaper |
| `wallet` / `credit` | card / piggy-bank |
| `quote` / `order` / `delivery` | text-bubble / list / car |
| `products` / `ticket` | archive / qr-code |
| `admin`, `cog` / `events` / `broker` / `globe` | gears / clock / link / globe |
| `eye` / `pencil` / `trash` / `file` / `print` | external-link / pen / trash / paper / newspaper |
| `panel` / `chevron` / `logout` | menu / chevron-down / logout |

`tools/fetch_papicons.py` regenerates the module from the set (it caches the SVGs in
`.paplicons_tmp/`), so updating the icons is one command.

### Two header rows

Every table in a page opens with two header rows. The first spans all columns and
holds the page's tools — the search box first, then `+ New` — centred; the second
names the columns, Arabic stacked above the Latin. `views::list_table()` and
`views::list_table_actions()` build the table, so a list cannot drift from the
house style by rendering its own toolbar.

```
<thead>
  <tr class="thead-tools"><th colspan="6">  search · + New  </th></tr>
  <tr><th>الرقم NUMBER</th> … </tr>
</thead>
```

Because the box lives in the header, it is in the same card as the table (and
scrolls sideways with it), and it stays there even when a search matches nothing:
the message moves into the body as a row spanning the columns, so the term can be
seen and cleared instead of vanishing with the table.

Both parts come from `pages::search_form()`: the same box, placeholder and button
everywhere, with the term still in it after the search. Search runs on the server.
`pages::list_filter()` builds the Mongo filter — the company's rows, narrowed to
those whose columns hold the term as literal, case-insensitive text — so each list
names the columns it searches:

| list | columns searched |
| --- | --- |
| Products | name, SKU, category (plus the low-stock filter) |
| Suppliers / Customers | name, code, email |
| Purchase orders | number, supplier |
| Goods receipts | number, purchase order, supplier |
| Supplier invoices | number, supplier |
| Quotes / Customer orders | number, customer |
| Delivery notes | number, customer, order |
| Customer invoices | number, customer |
| Payments | number, party, invoice |
| Tickets | number, cashier |
| Credit | customer name, code |
| Event log | routing key, event type, date |
| Broker queues | queue name |

The last three are reports: they have no `+ New` because none of them can create a
record, and their rows are already in hand, so they filter with
`pages::row_matches()` rather than a query.

### Settings › Font size

Text size is set **per font**, in a submenu for each face: `Comfortaa` (the Latin
half of every label) and `VIP Rawy` (the Arabic half), each with four steps:

| step | scale |
| --- | --- |
| `S` | 100 % |
| `M` | 150 % (default) |
| `L` | 200 % |
| `XL` | 250 % |

They are independent, so Arabic can be enlarged while the Latin text keeps its size.
The sizes are cookie-backed (`si_fs_lat`, `si_fs_ar`, values `s`/`m`/`l`/`xl`) and
implemented with `size-adjust` font faces, so a single bilingual label scales each
half separately.

### Settings › Icon size

One step for every glyph the app draws, so the rail, the header buttons, the settings
gear and the round row actions grow together instead of one part swelling out of
proportion:

| step | multiplier | nav icon | row-action icon |
| --- | --- | --- | --- |
| `S` | 0.85 | 14.5 px | 12.75 px |
| `M` | 1 (default) | 17 px | 15 px |
| `L` | 1.2 | 20.4 px | 18 px |
| `XL` | 1.4 | 23.8 px | 21 px |

The step is a cookie (`si_ico`, values `s`/`m`/`l`/`xl`), set by `/icon/{size}` and
read into `Ctx::ico`, which the shell writes onto `<html data-ico="…">`. `app.css`
turns it into `--ico`, and every icon rule is `calc(<design size> * var(--ico))`, so
a frame keeps the proportion it was drawn at.

## Fonts

The UI font is **Comfortaa** for Latin and **VIP Rawy** for Arabic; the PDF
writer uses `arabic.ttf` for Arabic documents:

```
static/fonts/Comfortaa-{Light,Regular,Bold}.ttf   # UI, Latin (OFL, bundled)
static/fonts/VIP-Rawy-{Regular,Bold}.ttf          # UI, Arabic (commercial)
static/fonts/arabic.ttf                            # PDF, Arabic (Amiri, OFL, bundled)
```

They are served from `/static/fonts/<file>` and declared in `static/app.css` with
`@font-face`, including the true 300/400/700 weights. Missing files fall back to
the system stack, so a checkout without them still works.

**Numbers follow Comfortaa in every language.** VIP Rawy also carries Latin
digits, so the page stack puts Comfortaa first and VIP Rawy second: Latin letters
and every digit, decimal point and thousands comma are drawn by Comfortaa, while the
Arabic letters fall through to VIP Rawy. The numeric stack (`--font-num`, used by
`.num` cells, totals, aging rows and the version pill) is the Comfortaa face too, so
amounts look identical whichever duo is selected.

**Text size** is set per face in `Settings › Font size`, through `Comfortaa S/M/L/XL`
and `VIP Rawy S/M/L/XL` — the same files again with `size-adjust` at 100 %, 150 %,
200 % and 250 %. The Arabic variants are limited to the Arabic Unicode blocks, so
raising the Arabic size never touches Latin letters or digits, and the step applies
to both faces equally, so the duo keeps its balance at every size.

VIP Rawy is a commercial face: it is **not committed** (see `.gitignore`), copy
it into `static/fonts/` for each deployment if you are licensed for it. The PDF
side is independent — to use VIP Rawy in PDFs as well, either overwrite
`static/fonts/arabic.ttf` with it or set `PDF_ARABIC_FONT`; the writer only needs
a TrueType file with Arabic presentation forms. Amiri ships under the SIL Open
Font License (`static/fonts/OFL.txt`); without it Arabic PDFs fall back to the
English/French labels.

## Cash Registry

The till sits in the sidebar straight under **Dashboard**, at `/cash-registry`, and it is
where a day of selling starts: the cashier picks the customer, adds what leaves the
shelf, takes the cash and hands over a printed receipt. The **New sale** panel is laid
out like a calculator, because that is what the counter is: the cart on the left, and on
the right a screen with the total, the amount received and the change, with the keypad
right under the thumb. On a narrow screen the screen and its keys drop under the cart.

- **The customer.** A dropdown of the company's customers, plus *Walk-in customer* for
the sale that has no name to it. The name and id are written on the ticket, so the same
sale can be found from the customer's side later.
- **The shelf.** Every product line picks from stock and shows the price and the
  quantity on hand next to its name; choosing one fills the price and the description.
  Charging writes one `Ticket` and `services::post_ticket` takes the quantity out of
  `qty_on_hand` with a `ticket` stock move, exactly as a direct ticket does.
- **The cash.** The total sits big on the screen, with the subtotal and tax under it,
  recomputed on every line and every key. The amount received is tapped on the keypad
  (0–9, `.`, backspace, *Clear*) or typed, the quick amounts (20 / 50 / 100 / 200 and
  *Exact*) fill it in one tap, and the change follows live — in red while the cash on
  the counter is still short of the total. A short payment is refused by the server
  with a red notice, not written down as a debt.
- **The credit.** *Payment* switches the sale from cash to credit: the goods still
  leave the shelf today, but the money is owed until the invoice falls due — the last
  day of the month by default, changeable on the form. A credit sale writes a customer
  invoice (`CI-…`, status open) instead of a ticket, so it lands straight in
  **Customer Invoices**, the customer's balance and the **Credit** page, and is settled
  later with *Record payment* like any other invoice. It is refused in red when the
  customer has no name to be owed by (a walk-in), or when the amount would push the
  customer's exposure past their **credit limit** — `0` still means unlimited. The head
  of the page counts what went out on credit today apart from the cash in the drawer.
- **The printer.** A charged sale lands on its ticket, which opens its print dialog
  straight away; the page also carries *Print*, *Bluetooth print* (the ESC/POS over the
  BLE service `0x18F0`) and the PDF. The customer's name is part of the ESC/POS and PDF
  layouts when there is one.

The top of the page is the day's drawer — cash taken, receipts written, items sold and
the average basket — and under the form sits every receipt of the day with its time,
customer, total and change, each with a print circle. Direct tickets still exist for the
sale that never needs a name; both feed the same counters and the same stock.

## Direct ticket printing

`GET /tickets/{id}/escpos` streams raw ESC/POS bytes (init, 42-column layout,
totals, cash drawer, cut).

- **Cable/USB**: with the printer as a virtual COM port,
  `copy /b ticket.bin COM3` (Windows) or `cat ticket.bin > /dev/usb/lp0` (Linux).
- **Bluetooth**: the ticket page has a *Bluetooth print* button that pairs a
  BLE printer exposing the standard ESC/POS service `0x18F0` and writes the same
  bytes.
- A4 receipt preview: `GET /tickets/{id}/pdf` (80 mm-wide page) or `Ctrl+P` on
  the ticket page.

---

## Android app

The phone client lives in [`android/`](android/README.md): a native Java app, no WebView
and no third-party SDK.

**It stands alone.** A fresh install opens on its own SQLite database and needs no
server: suppliers, customers and products are entered on the phone, kept on the phone,
and the dashboard counts them. Settings — the link on the sign-in screen, or *Settings*
in the header — offers the same two database options as the web app:

| Option | Meaning |
|--------|---------|
| **Local — this phone** | The app's own database; everything works with no network |
| **Server — IP address** | Reads the office computer's `/api/v1` over the LAN, after signing in |

The choice lives in a plain text file, so it can be changed later without a rebuild:

```
Android/data/ma.s2c.maroc/files/s2c.properties

  db.mode=local            # local | server
  server.url=http://10.0.2.2:8080
  company.name=S2C MAROC
  company.currency=MAD
```

```bash
bash tools/build_android.sh                       # → android/dist/s2c-maroc-0.4.0-debug.apk
bash tools/build_android.sh assembleRelease       # → android/dist/s2c-maroc-0.4.0-release.apk
adb install -r android/dist/s2c-maroc-0.4.0-debug.apk
```

The app opens on a sign-in page with three tabs - Login, Forgotten password and Settings:
the first run creates the phone's own account, and a recovery phrase resets a forgotten
password. Under it sits a single screen with the till and five tabs — **Cashier**, **Products**, **Suppliers**,
**Customers**, **Credit**. A sale is paid cash or put on credit, stock comes down with it,
and the credit book settles a customer in one long press. In local mode a list has a
**+ Add** button and a long-press deletes a row. The emulator
reaches this machine on `http://10.0.2.2:8080`; a phone needs the PC's LAN address, e.g.
`http://192.168.1.20:8080`. Documents — purchase orders, invoices, tickets — and
Bluetooth printing are still web-only and listed in the client's README.

## API (Android client)

The JSON API lives under `/api/v1`, authenticated with a Bearer token from
`POST /api/v1/auth/login`. See [`docs/API.md`](docs/API.md).

```bash
TOKEN=$(curl -s -X POST localhost:8080/api/v1/auth/login \
  -H 'content-type: application/json' \
  -d '{"username":"admin","password":"looklook"}' | jq -r .token)

curl -s localhost:8080/api/v1/dashboard -H "Authorization: Bearer $TOKEN"
```

---

## Testing

```bash
cargo test          # unit tests (money/format helpers, table + badge rendering, ESC/POS)
cargo check         # typecheck
```

Manual smoke path used during development (all steps verified against a live
MongoDB + RabbitMQ): sign in → supplier/customer/product → PO → send →
goods receipt (stock +10) → supplier invoice → issue → payment (partial) →
PDF; quote → validate → order + invoice + PDF → delivery note → post (stock −5);
POS ticket (stock −2, ESC/POS + PDF); credit limit rejection; cross-site POST
rejection; tenant isolation; EN/FR/AR switching.

---

## Known limitations

- **The Android app writes master data, not documents.** Suppliers, customers and
  products can be added and deleted in local mode; purchase orders, invoices, tickets,
  document actions and Bluetooth printing are still web-only.
- **The phone's local database does not sync.** Local and Server are two separate
  datasets: switching the mode switches which one the phone shows, with no merge.
- **The APK is debug-signed.** Fine for a phone on your desk, not for a store.
- Arabic PDFs keep the left-to-right grid (column order, totals block); the
  text itself is shaped and runs right to left, but the layout is not mirrored.
- PDF fonts are embedded whole (no subsetting), so an Arabic document is ~210 KB
  larger than a Latin one.
- One company and one profile per installation, per the specification.
- `SECRET_KEY`/`JWT_SECRET_KEY` shorter than 32 characters produce an ephemeral
  key at boot: sessions become invalid on restart.
- MongoDB in `docker-compose.yml` runs without auth for local development; put it
  behind a private network (or enable auth) for shared deployments.

---

## RabbitMQ log noise

The broker logs a lot on every boot; none of it is a fault. What the usual lines mean:

| Line | Meaning |
|------|---------|
| `alarm_handler: {set,{system_memory_high_watermark,[]}}` / `clear` | Erlang raises the memory alarm while the VM boots, then clears it. Publishers are only blocked if the watermark stays set (it is 40% of RAM, here 3137 MiB of 7844 MiB). |
| `Application syslog exited`, `Logging: switching to configured handler(s)` | The broker replaces its log handlers during startup. |
| `Khepri leader elected`, `Waiting for Khepri projections`, `ra: starting system …`, `WAL init` | RabbitMQ 3.13 metadata store (Khepri/Ra) coming up. Normal on a single node. |
| `Classic peer discovery backend: list of nodes does not contain the local node []` | Warning only — no cluster peers are configured. |
| `Message store … rebuilding indices from scratch` | First boot, or the container was recreated with a fresh volume. |
| `Deprecated features: management_metrics_collection` | The management plugin still enables a deprecated feature; harmless in 3.13. |
| `accepting AMQP connection` / `user 'stockinvoice' authenticated` | The app connected successfully. |
| `client unexpectedly closed TCP connection` | The app process stopped (killed, rebuilt, or restarted) without closing the AMQP socket. |
| `SIGTERM received - shutting down`, `broker forced connection closure … 'shutdown'` | The container was stopped/restarted (`docker compose restart`, Docker Desktop restart). |
| `HTTP listener registry could not find context … _tls` | Cosmetic shutdown message when no TLS listener is configured. |

So a pair of boot sequences in the log simply means the RabbitMQ container was
recreated; readiness is `Server startup complete` plus `rabbitmq-diagnostics ping`
(the compose healthcheck does exactly that).
