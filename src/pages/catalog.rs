//! Products, stock levels, movements and manual adjustments.

use axum::extract::{Path, Query};
use axum::response::{IntoResponse, Response};
use futures_util::TryStreamExt;
use mongodb::bson::doc;
use std::collections::HashMap;

use crate::auth::Ctx;
use crate::error::AppError;
use crate::form::Pairs;
use crate::i18n::tr;
use crate::models::{MoveKind, Product, StockMove};
use crate::services;
use crate::util::{esc, now_iso, qty};
use crate::views;

pub async fn products(ctx: Ctx, Query(q): Query<HashMap<String, String>>) -> Result<Response, AppError> {
    let search = q.get("q").cloned().unwrap_or_default();
    let low_only = q.get("low").map(|v| v == "1").unwrap_or(false);

    let filter = if search.trim().is_empty() {
        doc! { "company_id": &ctx.company.id }
    } else {
        let re = mongodb::bson::Regex {
            pattern: crate::pages::regex_escape(search.trim()),
            options: "i".into(),
        };
        doc! {
            "company_id": &ctx.company.id,
            "$or": [
                { "name": { "$regex": re.clone() } },
                { "sku": { "$regex": re.clone() } },
                { "category": { "$regex": re } },
            ]
        }
    };
    let mut cur = ctx
        .state
        .db
        .products()
        .find(filter)
        .sort(doc! { "name": 1 })
        .await?;
    let l = ctx.lang;
    let cur_code = ctx.company.currency.clone();
    let mut rows = Vec::new();
    let mut stock_value = 0.0;
    while let Some(p) = cur.try_next().await? {
        stock_value += p.qty_on_hand.max(0.0) * p.cost;
        if low_only && !p.is_low() {
            continue;
        }
        let stock_cell = format!(
            "<span class=\"num {}\">{}</span>",
            if p.is_low() { "neg" } else { "" },
            qty(p.qty_on_hand)
        );
        rows.push(vec![
            format!(
                "<a href=\"/products/{}/edit\">{}</a><div class=\"muted small\">{}</div>",
                esc(&p.id),
                esc(&p.name),
                esc(&p.sku)
            ),
            esc(&p.category),
            esc(&p.unit),
            format!("<span class=\"num\">{}</span>", esc(&views::money_text(p.cost, &cur_code))),
            format!("<span class=\"num\">{}</span>", esc(&views::money_text(p.price, &cur_code))),
            stock_cell,
            views::icon_link(&format!("/products/{}/movements", p.id), "events", &tr(l, "nav_events")),
        ]);
    }

    let body = format!(
        "{}{}",
        super::page_head(
            &ctx,
            "nav_products",
            &format!(
                "{}: {}",
                tr(l, "m_stock_value"),
                views::money_text(stock_value, &cur_code)
            ),
            vec![
                format!(
                    "<form class=\"search\" method=\"get\" action=\"/products\"><input name=\"q\" value=\"{}\" placeholder=\"{}\"/><label class=\"mini-check\"><input type=\"checkbox\" name=\"low\" value=\"1\"{} onchange=\"this.form.submit()\"/>{}</label><button class=\"btn btn-sm\" type=\"submit\">{}</button></form>",
                    esc(&search),
                    crate::i18n::tr_text(l, "lbl_search"),
                    if low_only { " checked" } else { "" },
                    tr(l, "m_low_stock"),
                    tr(l, "a_search")
                ),
                views::link_btn("/products/new", &format!("+ {}", tr(l, "a_new"))),
            ]
        ),
        views::table_actions(
            l,
            &[
                &tr(l, "lbl_product"),
                &tr(l, "lbl_category"),
                &tr(l, "lbl_unit"),
                &tr(l, "lbl_cost"),
                &tr(l, "lbl_price"),
                &tr(l, "lbl_stock"),
                &tr(l, "lbl_actions"),
            ],
            rows,
            "msg_no_records",
        )
    );
    let title = views::Title::of(l, "nav_products");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

fn product_fields(l: crate::i18n::Lang, p: &Product, with_stock: bool) -> String {
    let mut html = format!(
        "<div class=\"form-grid\">{}{}{}{}{}{}{}{}</div>",
        views::input(&tr(l, "lbl_sku"), "sku", &p.sku, "text", true),
        views::input(&tr(l, "lbl_name"), "name", &p.name, "text", true),
        views::input(&tr(l, "lbl_unit"), "unit", &p.unit, "text", false),
        views::input(&tr(l, "lbl_category"), "category", &p.category, "text", false),
        views::input_step(&tr(l, "lbl_cost"), "cost", &format!("{:.2}", p.cost), "0.01", false),
        views::input_step(&tr(l, "lbl_price"), "price", &format!("{:.2}", p.price), "0.01", false),
        views::input_step(&tr(l, "lbl_reorder"), "reorder_level", &qty(p.reorder_level), "0.01", false),
        views::checkbox(&tr(l, "lbl_active"), "active", p.active),
    );
    if with_stock {
        html.push_str(&views::input_step(
            &tr(l, "lbl_stock"),
            "qty_on_hand",
            &qty(p.qty_on_hand),
            "0.01",
            false,
        ));
    }
    html
}

pub async fn product_new(ctx: Ctx) -> Result<Response, AppError> {
    let l = ctx.lang;
    let empty = Product {
        active: true,
        unit: "pcs".into(),
        ..Default::default()
    };
    let body = format!(
        "{}<form method=\"post\" action=\"/products\" class=\"card form\">{} {}</form>",
        super::page_head(&ctx, "nav_products", "", vec![]),
        product_fields(l, &empty, true),
        views::form_actions(&tr(l, "a_save"), "/products", &tr(l, "a_back")),
    );
    let title = views::Title::of(l, "nav_products").join(l, "a_new");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn product_create(ctx: Ctx, pairs: Pairs) -> Result<Response, AppError> {
    let now = now_iso();
    let p = Product {
        id: crate::util::new_id(),
        company_id: ctx.company.id.clone(),
        sku: pairs.get("sku"),
        name: pairs.get("name"),
        unit: crate::util::if_empty(&pairs.get("unit"), "pcs"),
        category: pairs.get("category"),
        cost: pairs.num("cost", 0.0),
        price: pairs.num("price", 0.0),
        qty_on_hand: pairs.num("qty_on_hand", 0.0),
        reorder_level: pairs.num("reorder_level", 0.0),
        active: pairs.flag("active"),
        created_at: now.clone(),
        updated_at: now,
    };
    if p.name.trim().is_empty() || p.sku.trim().is_empty() {
        return Ok(ctx.back("/products/new", "msg_required", "err").into_response());
    }
    match ctx.state.db.products().insert_one(&p).await {
        Ok(_) => {
            if p.qty_on_hand != 0.0 {
                let mv = StockMove {
                    id: crate::util::new_id(),
                    company_id: p.company_id.clone(),
                    product_id: p.id.clone(),
                    product_name: p.name.clone(),
                    kind: MoveKind::Adjust,
                    qty: p.qty_on_hand,
                    unit_cost: p.cost,
                    source: "opening".into(),
                    source_number: String::new(),
                    note: "Opening stock".into(),
                    created_at: p.created_at.clone(),
                    created_by: ctx.claims.un.clone(),
                };
                ctx.state.db.stock_moves().insert_one(&mv).await?;
            }
            Ok(ctx.back("/products", "msg_saved", "ok").into_response())
        }
        Err(e) if e.to_string().contains("duplicate key") => {
            Ok(ctx.back("/products/new", "msg_duplicate", "err").into_response())
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn product_edit(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let p = services::get_by_id::<Product>(&ctx.state.db.products(), &ctx.company.id, &id).await?;
    let body = format!(
        "{}<form method=\"post\" action=\"/products/{}\" class=\"card form\">{}{}</form>",
        super::page_head(&ctx, "nav_products", &format!("{} · {}", p.sku, p.name), vec![]),
        esc(&p.id),
        product_fields(l, &p, false),
        views::form_actions(&tr(l, "a_save"), "/products", &tr(l, "a_back")),
    );
    let title = views::Title::of(l, "a_edit");
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn product_update(ctx: Ctx, Path(id): Path<String>, pairs: Pairs) -> Result<Response, AppError> {
    let mut p = services::get_by_id::<Product>(&ctx.state.db.products(), &ctx.company.id, &id).await?;
    p.sku = pairs.get("sku");
    p.name = pairs.get("name");
    p.unit = crate::util::if_empty(&pairs.get("unit"), "pcs");
    p.category = pairs.get("category");
    p.cost = pairs.num("cost", p.cost);
    p.price = pairs.num("price", p.price);
    p.reorder_level = pairs.num("reorder_level", p.reorder_level);
    p.active = pairs.flag("active");
    p.updated_at = now_iso();
    ctx.state
        .db
        .products()
        .replace_one(doc! { "_id": &p.id, "company_id": &ctx.company.id }, &p)
        .await?;
    Ok(ctx.back("/products", "msg_saved", "ok").into_response())
}

pub async fn product_delete(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    ctx.state
        .db
        .products()
        .delete_one(doc! { "_id": &id, "company_id": &ctx.company.id })
        .await?;
    Ok(ctx.back("/products", "msg_deleted", "ok").into_response())
}

pub async fn product_movements(ctx: Ctx, Path(id): Path<String>) -> Result<Response, AppError> {
    let l = ctx.lang;
    let p = services::get_by_id::<Product>(&ctx.state.db.products(), &ctx.company.id, &id).await?;
    let mut cur = ctx
        .state
        .db
        .stock_moves()
        .find(doc! { "company_id": &ctx.company.id, "product_id": &p.id })
        .sort(doc! { "created_at": -1 })
        .limit(200)
        .await?;
    let mut rows = Vec::new();
    while let Some(mv) = cur.try_next().await? {
        rows.push(vec![
            esc(&mv.created_at),
            format!(
                "<span class=\"badge {}\">{}</span>",
                if mv.kind == MoveKind::In { "green" } else { "amber" },
                match mv.kind {
                    MoveKind::In => tr(l, "lbl_received"),
                    MoveKind::Out => tr(l, "nav_delivery_notes"),
                    MoveKind::Adjust => tr(l, "a_adjust"),
                }
            ),
            format!(
                "<span class=\"num\">{}{}</span>",
                if mv.kind == MoveKind::In { "+" } else { "-" },
                qty(mv.qty.abs())
            ),
            esc(&mv.source_number),
            esc(&mv.created_by),
        ]);
    }

    let body = format!(
        "{}{}{}",
        super::page_head(
            &ctx,
            "nav_products",
            &format!("{} · {} {}", p.name, qty(p.qty_on_hand), p.unit),
            vec![views::link_btn(&format!("/products/{}/edit", p.id), &tr(l, "a_edit"))]
        ),
        views::card(
            &tr(l, "a_adjust"),
            format!(
                "<form method=\"post\" action=\"/products/{}/adjust\" class=\"inline-form\">\
                   <label class=\"field\"><span>{}</span><input name=\"qty\" type=\"number\" step=\"0.01\" value=\"0\" required/></label>\
                   <label class=\"field\"><span>{}</span><input name=\"note\"/></label>\
                   <button class=\"btn btn-primary\" type=\"submit\">{}</button>\
                 </form>",
                esc(&p.id),
                tr(l, "lbl_quantity"),
                tr(l, "lbl_notes"),
                tr(l, "a_apply"),
            )
        ),
        views::card(
            &tr(l, "nav_events"),
            views::table(
                l,
                &[
                    &tr(l, "lbl_date"),
                    &tr(l, "lbl_status"),
                    &tr(l, "lbl_quantity"),
                    &tr(l, "lbl_document"),
                    &tr(l, "lbl_created_by"),
                ],
                rows,
                "msg_no_records",
            )
        )
    );
    let title = views::Title::raw(&p.name);
    Ok(views::respond(views::page(&ctx, &title, body)))
}

pub async fn product_adjust(ctx: Ctx, Path(id): Path<String>, pairs: Pairs) -> Result<Response, AppError> {
    let p = services::get_by_id::<Product>(&ctx.state.db.products(), &ctx.company.id, &id).await?;
    let delta = pairs.num("qty", 0.0);
    if delta == 0.0 {
        return Ok(ctx
            .back(&format!("/products/{id}/movements"), "msg_invalid_amount", "err")
            .into_response());
    }
    let now = now_iso();
    ctx.state
        .db
        .products()
        .update_one(
            doc! { "_id": &p.id, "company_id": &ctx.company.id },
            doc! { "$inc": { "qty_on_hand": delta }, "$set": { "updated_at": &now } },
        )
        .await?;
    let mv = StockMove {
        id: crate::util::new_id(),
        company_id: ctx.company.id.clone(),
        product_id: p.id.clone(),
        product_name: p.name.clone(),
        kind: MoveKind::Adjust,
        qty: delta,
        unit_cost: p.cost,
        source: "adjustment".into(),
        source_number: String::new(),
        note: pairs.get("note"),
        created_at: now,
        created_by: ctx.claims.un.clone(),
    };
    ctx.state.db.stock_moves().insert_one(&mv).await?;
    Ok(ctx
        .back(&format!("/products/{id}/movements"), "msg_stock_adjusted", "ok")
        .into_response())
}
