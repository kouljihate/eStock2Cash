//! Raw form handling. Browser forms submit repeated keys (`item_product`,
//! `item_qty`, ...) for dynamic document lines, which `serde_urlencoded`'s
//! struct mapping cannot express — so we parse the pairs and zip them by index.

use std::collections::HashMap;

use axum::body::Bytes;
use axum::extract::{FromRequest, Request};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::models::{DocLine, Product, TicketItem};
use crate::util::round2;

#[derive(Debug, Clone, Default)]
pub struct Pairs(pub Vec<(String, String)>);

#[derive(Debug)]
pub struct FormError(pub String);

impl IntoResponse for FormError {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, format!("invalid form data: {}", self.0)).into_response()
    }
}

impl<S: Send + Sync> FromRequest<S> for Pairs {
    type Rejection = FormError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|e| FormError(e.to_string()))?;
        if bytes.is_empty() {
            return Ok(Pairs(Vec::new()));
        }
        let pairs: Vec<(String, String)> = serde_urlencoded::from_bytes(&bytes)
            .map_err(|e| FormError(e.to_string()))?;
        Ok(Pairs(pairs))
    }
}

impl Pairs {
    pub fn get(&self, key: &str) -> String {
        self.0
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.trim().to_string())
            .unwrap_or_default()
    }

    pub fn opt(&self, key: &str) -> Option<String> {
        let v = self.get(key);
        if v.is_empty() { None } else { Some(v) }
    }

    pub fn num(&self, key: &str, default: f64) -> f64 {
        let v = self.get(key);
        if v.is_empty() {
            default
        } else {
            v.replace(',', ".").parse::<f64>().unwrap_or(default)
        }
    }

    pub fn int(&self, key: &str, default: i64) -> i64 {
        let v = self.get(key);
        if v.is_empty() {
            default
        } else {
            v.parse::<i64>().unwrap_or(default)
        }
    }

    pub fn flag(&self, key: &str) -> bool {
        matches!(self.get(key).as_str(), "on" | "1" | "true" | "yes")
    }

    pub fn all(&self, key: &str) -> Vec<String> {
        self.0
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.trim().to_string())
            .collect()
    }

}

fn at(values: &[String], i: usize) -> String {
    values.get(i).cloned().unwrap_or_default()
}

/// Parse the dynamic `item_*` rows into document lines.
///
/// `use_cost` picks the product's cost (purchases) or sale price (sales) when
/// the user leaves the price empty.
pub fn parse_doc_lines(
    p: &Pairs,
    products: &HashMap<String, Product>,
    use_cost: bool,
    default_tax: f64,
) -> Vec<DocLine> {
    let prod_ids = p.all("item_product");
    let descs = p.all("item_desc");
    let qtys = p.all("item_qty");
    let prices = p.all("item_price");
    let taxes = p.all("item_tax");

    let mut out = Vec::new();
    for i in 0..prod_ids.len() {
        let product_id = at(&prod_ids, i);
        let mut description = at(&descs, i);
        let qty_raw = at(&qtys, i).replace(',', ".");
        let qty = qty_raw.parse::<f64>().unwrap_or(0.0);
        let price_raw = at(&prices, i).replace(',', ".");
        let mut unit_price = price_raw.parse::<f64>().unwrap_or(0.0);
        let tax_raw = at(&taxes, i).replace(',', ".");
        let tax_rate = tax_raw.parse::<f64>().unwrap_or(default_tax);

        if description.is_empty() && product_id.is_empty() {
            continue;
        }
        if let Some(prod) = products.get(&product_id) {
            if description.is_empty() {
                description = prod.name.clone();
            }
            if unit_price <= 0.0 {
                unit_price = if use_cost { prod.cost } else { prod.price };
            }
        }
        if qty <= 0.0 {
            continue;
        }
        out.push(DocLine {
            product_id,
            description,
            qty,
            unit_price: round2(unit_price),
            tax_rate,
        });
    }
    out
}

pub fn parse_ticket_items(p: &Pairs, products: &HashMap<String, Product>) -> Vec<TicketItem> {
    let ids = p.all("item_product");
    let names = p.all("item_desc");
    let qtys = p.all("item_qty");
    let prices = p.all("item_price");

    let mut out = Vec::new();
    for i in 0..ids.len() {
        let product_id = at(&ids, i);
        let mut name = at(&names, i);
        let qty = at(&qtys, i).replace(',', ".").parse::<f64>().unwrap_or(0.0);
        let mut price = at(&prices, i).replace(',', ".").parse::<f64>().unwrap_or(0.0);
        if name.is_empty() && product_id.is_empty() {
            continue;
        }
        if let Some(prod) = products.get(&product_id) {
            if name.is_empty() {
                name = prod.name.clone();
            }
            if price <= 0.0 {
                price = prod.price;
            }
        }
        if qty <= 0.0 {
            continue;
        }
        out.push(TicketItem {
            product_id,
            name,
            qty,
            price: round2(price),
        });
    }
    out
}

/// Load the company's products keyed by id (used to fill defaults and names).
pub async fn product_index(
    db: &crate::state::DbHandle,
    company_id: &str,
) -> Result<HashMap<String, Product>, crate::error::AppError> {
    use futures_util::TryStreamExt;
    use mongodb::bson::doc;
    let mut cur = db
        .products()
        .find(doc! { "company_id": company_id })
        .await?;
    let mut map = HashMap::new();
    while let Some(p) = cur.try_next().await? {
        map.insert(p.id.clone(), p);
    }
    Ok(map)
}
