# S2C MAROC API v1

JSON API for the Android client. All endpoints are under `http://<host>:8080/api/v1`.

- Authentication: `Authorization: Bearer <token>` (token from `/auth/login`).
- Every response is scoped to the company embedded in the token; ids from other
  companies are treated as not found.
- Errors: `{"ok": false, "error": "message", "code": 400}` with a matching HTTP status.

## Auth

### `POST /auth/login`

```json
{ "username": "admin", "password": "looklook" }
```

```json
{
  "ok": true,
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "expires_in": 3600,
  "user": { "id": "6aec...", "username": "admin", "full_name": "Local Administrator", "role": "local_admin", "locale": "en" },
  "company": { "id": "6aec...", "name": "S2C MAROC", "currency": "MAD" }
}
```

`401` on bad credentials or an inactive user.

### `GET /me`

Returns the user and company documents.

### `GET /health`

Public liveness probe: `{"ok": true, "service": "stockinvoice", "version": "0.10.1"}`.

## Dashboard

### `GET /dashboard`

```json
{
  "ok": true, "suppliers": 3, "customers": 5, "products": 40, "low_stock": 2,
  "stock_value": 1050.0, "receivables": 94.5, "payables": 60.0,
  "open_purchase_orders": 1, "open_quotes": 2,
  "aging": { "current": 94.5, "d30": 0, "d60": 0, "d90": 0, "total": 94.5 }
}
```

## Master data

| Method | Path | Notes |
|--------|------|-------|
| GET | `/suppliers?q=` | list, search by name/code/email |
| POST | `/suppliers` | `{code, name, email?, phone?, address?, city?, tax_id?}` |
| GET | `/customers?q=` | includes `credit_limit`, `exposure` |
| POST | `/customers` | `{code, name, credit_limit?, payment_terms?, email?, phone?, address?, city?}` |
| GET | `/products?q=` | includes stock and prices |
| POST | `/products` | `{sku, name, unit?, category?, cost?, price?, qty_on_hand?, reorder_level?}` |

List responses: `{"ok": true, "count": 2, "items": [ … ]}`.
Create responses: `{"ok": true, "item": { … }}`. Duplicate `code`/`sku` returns `500`
with the database error message.

## Documents

| Method | Path | Notes |
|--------|------|-------|
| GET | `/purchase-orders` | newest first |
| GET | `/purchase-orders/{id}` | with lines and totals |
| GET | `/supplier-invoices` | newest first |
| GET | `/customer-invoices` | newest first |
| GET | `/customer-invoices/{id}` | adds `outstanding` and effective `status` |
| GET | `/customer-invoices/{id}/pdf` | `application/pdf` |
| GET | `/quotes` | newest first |
| GET | `/customer-orders` | newest first |
| GET | `/payments` | newest first |

Document objects use the stored representation, e.g.:

```json
{
  "id": "6aec...", "number": "CI-2026-0001", "party_id": "6aec...",
  "party_name": "Bright Retail", "invoice_date": "2026-11-05",
  "due_date": "2026-12-05", "status": "open", "paid_amount": 0,
  "totals": { "subtotal": 90.0, "tax": 4.5, "total": 94.5 },
  "lines": [ { "product_id": "6aec...", "description": "Widget", "qty": 5, "unit_price": 18, "tax_rate": 5 } ]
}
```

## Tickets (POS)

### `GET /tickets`

Last 100 direct-purchase tickets.

### `POST /tickets`

```json
{
  "items": [
    { "product_id": "6aec...", "qty": 2, "price": 18 },
    { "name": "Coffee", "qty": 1, "price": 3.5 }
  ],
  "tax": 1.0,
  "paid": 50.0,
  "cashier": "admin"
}
```

`product_id` may be empty for a free-text item (then `name` is required). When a
product is given, its name/price fill the missing fields. The ticket is created
with the next `TK-…` number and stock is decreased for product lines.

A ticket also carries `customer_id` and `customer_name`. The till (`/cash-registry`)
fills them from the customer it was given; this endpoint leaves them empty, which is
the same shape as a walk-in sale.

Response: `{"ok": true, "item": { … ticket … }}`.

## Credit

### `GET /credit`

```json
{
  "ok": true,
  "customers": [ { "id": "…", "name": "Bright Retail", "limit": 5000, "exposure": 94.5, "available": 4905.5 } ],
  "aging": { "current": 94.5, "d30": 0, "d60": 0, "d90": 0, "total": 94.5 }
}
```

`available` is `null` for unlimited credit (limit `0`).

## Not exposed (web only)

Creating/posting documents, payments, quote validation and admin settings are
web-form endpoints (`/purchase-orders`, `/goods-receipts`, `/quotes/{id}/accept`,
`/payments`, `/admin`, …). They are plain form posts protected by the session
cookie and same-origin checks; a mobile client cannot call them with the JSON API
token yet.

The till is a web form too: `POST /cash-registry` takes the customer, the product lines
and the cash received, writes a ticket and redirects to its receipt. A short payment is
refused (`?m=msg_paid_short&k=err`), a sale with no line is refused
(`?m=msg_line_required&k=err`), and a completed one answers `?m=msg_cash_taken&k=ok` on
`/tickets/{id}`.

With `payment=credit` the same form sells on account instead: it writes an open
customer invoice (`CI-…`) due on `due_date` — the end of the month when left empty —
takes the stock out with a `credit_sale` stock move and lands on
`/customer-invoices/{id}?m=msg_credit_sale&k=ok`. It needs a customer
(`?m=msg_credit_needs_customer&k=err`) and is refused when the invoice would pass that
customer's `credit_limit` (`?m=msg_credit_exceeded&k=err`, `0` = unlimited).

Deleting a document is web-only for the same reason. `POST /purchase-orders/{id}/delete`
and `POST /supplier-invoices/{id}/delete` answer with a redirect carrying a flash code
(`?m=…&k=ok|err`) instead of JSON.

- Deleting a purchase order deletes its goods receipts as well. A receipt that was
  already posted gives its stock back first, which writes one `stock_moves` row per
  product (`kind: "out"`, `source: "goods_receipt_void"`) rather than erasing the
  ledger; a draft receipt simply goes away.
- Both deletes are refused while money hangs off the record: a purchase order billed
  on a supplier invoice answers `?m=msg_delete_blocked_invoices&k=err`, and an invoice
  with payments answers `?m=msg_delete_blocked_payments&k=err`. The record survives.
