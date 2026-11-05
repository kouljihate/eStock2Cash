# Changelog

All notable changes to StockInvoice. The version follows the crate version in
`Cargo.toml` and is shown at the bottom of the sidebar.

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
