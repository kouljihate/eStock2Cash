# StockInvoice

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
| **i18n** | English, French, Arabic (RTL) with switcher | Ready |
| **RabbitMQ** | Topic events, MongoDB mirror, management monitor | Ready |
| **Docker** | App + local MongoDB + local RabbitMQ | Ready |
| **Android app** | Client not built; the JSON API it needs is ready | Partial |

---

## Quick start

### Docker (app + MongoDB + RabbitMQ, all local)

```bash
docker compose up --build
# app        http://localhost:8080
# rabbitmq   http://localhost:15672  (stockinvoice / stockinvoice)
```

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
| `COMPANY_NAME` | `StockInvoice` | Seeded company name |
| `DEFAULT_LOCALE` | `en` | `en`, `fr` or `ar` |

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
  ticket.rs      ESC/POS byte stream for mini printers
  i18n.rs        EN/FR/AR dictionary
  views.rs       Leptos shell (sidebar, topbar), tables, form components
  pages/         one module per domain (handlers + rendered bodies)
  routes.rs      web routing
  api.rs         /api/v1 JSON API (Bearer JWT) for the Android client
static/          app.css, app.js, favicon (fonts are added by the operator)
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
  quote, confirming an order and issuing/creating a customer invoice.
- **Quote validation** creates a confirmed sales order *and* an open customer
  invoice in one step, and links both back to the quote.

### Events (RabbitMQ → MongoDB)

Every state change publishes to the `stockinvoice` topic exchange:
`po.created`, `po.sent`, `gr.posted`, `supplier_invoice.created`,
`supplier_invoice.issued`, `payment.recorded`, `quote.created`, `quote.sent`,
`quote.accepted`, `order.confirmed`, `delivery.posted`,
`customer_invoice.issued`, `ticket.created`.

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

- The writer is dependency-free Rust (Helvetica base-14 + WinAnsi), so no font
  embedding or external service is needed, and multi-page invoices paginate
  automatically.
- **Logo**: paste a base64 JPEG (`data:image/jpeg;base64,…` or the raw
  `/9j/…` form) into the company profile. PNG logos are ignored — PDF image
  embedding uses `DCTDecode`, which is JPEG-only.
- **Arabic**: the PDF uses English/French labels because Arabic needs an embedded
  shaped CID font (e.g. VIP RAWY). The on-screen document is fully translated.

## Fonts

The theme loads `Comfortaa` (Latin) and `VIP RAWY` (Arabic) if present:

```
static/fonts/Comfortaa-Regular.ttf
static/fonts/VIPRawy-Regular.ttf
```

Fonts are not bundled (licensing); without them the UI falls back to the system
font stack and Arabic still renders with a system Arabic face.

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

- **Android app is not included.** The API it needs (auth, dashboard, master
  data, documents, tickets, PDF) is implemented and documented.
- Arabic **PDF** labels are English/French (see above); the UI itself is fully
  Arabic with RTL.
- One company and one profile per installation, per the specification.
- `SECRET_KEY`/`JWT_SECRET_KEY` shorter than 32 characters produce an ephemeral
  key at boot: sessions become invalid on restart.
- MongoDB in `docker-compose.yml` runs without auth for local development; put it
  behind a private network (or enable auth) for shared deployments.
