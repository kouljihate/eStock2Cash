//! RabbitMQ integration: publish domain events, mirror them into MongoDB
//! (`RabbitMQ -> MongoDB`) and expose broker health for the admin panel.

use std::sync::Arc;

use futures_util::StreamExt;
use lapin::options::{
    BasicAckOptions, BasicConsumeOptions, ExchangeDeclareOptions, QueueBindOptions,
    QueueDeclareOptions,
};
use lapin::types::FieldTable;
use lapin::{Channel, Connection, ConnectionProperties, ExchangeKind};
use serde::Serialize;
use tokio::sync::Mutex;

use crate::config::Config;
use crate::models::{DomainEvent, EventLog};
use crate::state::DbHandle;
use crate::util::{now_iso, parse_iso};

pub const EXCHANGE: &str = "stockinvoice";
pub const QUEUE: &str = "stockinvoice.events";

#[derive(Clone)]
pub struct Bus {
    cfg: Arc<Config>,
    conn: Arc<Mutex<Option<(Connection, Channel)>>>,
}

impl Bus {
    pub fn new(cfg: Arc<Config>) -> Self {
        Self {
            cfg,
            conn: Arc::new(Mutex::new(None)),
        }
    }

    pub fn enabled(&self) -> bool {
        self.cfg.rabbit_url.is_some()
    }

    async fn channel(&self) -> Option<Channel> {
        let mut guard = self.conn.lock().await;
        if let Some((conn, ch)) = guard.as_ref() {
            if conn.status().connected() && ch.status().connected() {
                return Some(ch.clone());
            }
        }
        let url = self.cfg.rabbit_url.as_deref()?;
        match Connection::connect(url, ConnectionProperties::default()).await {
            Ok(conn) => match conn.create_channel().await {
                Ok(ch) => {
                    let _ = ch
                        .exchange_declare(
                            EXCHANGE.into(),
                            ExchangeKind::Topic,
                            ExchangeDeclareOptions {
                                durable: true,
                                ..Default::default()
                            },
                            FieldTable::default(),
                        )
                        .await;
                    *guard = Some((conn, ch.clone()));
                    Some(ch)
                }
                Err(e) => {
                    tracing::warn!(error = %e, "rabbitmq: channel creation failed");
                    None
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "rabbitmq: connection failed (events will be dropped until it is back)");
                None
            }
        }
    }

    /// Fire-and-forget publish: messaging never blocks or breaks a request.
    pub async fn publish(&self, routing_key: &str, event: &DomainEvent) {
        if !self.enabled() {
            return;
        }
        let Some(ch) = self.channel().await else { return };
        let payload = match serde_json::to_vec(event) {
            Ok(p) => p,
            Err(_) => return,
        };
        let res = ch
            .basic_publish(
                EXCHANGE.into(),
                routing_key.into(),
                Default::default(),
                &payload,
                lapin::BasicProperties::default()
                    .with_content_type("application/json".into())
                    .with_delivery_mode(2),
            )
            .await;
        match res {
            Ok(confirm) => {
                let confirmed = confirm.await;
                if confirmed.is_err() {
                    tracing::warn!(routing_key, "rabbitmq: publish not confirmed");
                }
            }
            Err(e) => tracing::warn!(routing_key, error = %e, "rabbitmq: publish failed"),
        }
    }

    #[allow(dead_code)]
    pub async fn connected(&self) -> bool {
        let guard = self.conn.lock().await;
        guard
            .as_ref()
            .map(|(c, ch)| c.status().connected() && ch.status().connected())
            .unwrap_or(false)
    }
}

pub fn new_event(
    event_type: &str,
    company_id: &str,
    document_id: &str,
    document_number: &str,
    status: &str,
    amount: f64,
    actor: &str,
) -> DomainEvent {
    DomainEvent {
        event_type: event_type.to_string(),
        company_id: company_id.to_string(),
        document_id: document_id.to_string(),
        document_number: document_number.to_string(),
        status: status.to_string(),
        amount,
        actor: actor.to_string(),
        at: now_iso(),
    }
}

/// Consume the topic and store every event in MongoDB. Reconnects forever.
pub fn spawn_consumer(db: DbHandle, cfg: Arc<Config>) {
    let Some(url) = cfg.rabbit_url.clone() else {
        tracing::info!("RABBITMQ_URL not set - event consumer disabled");
        return;
    };
    tokio::spawn(async move {
        loop {
            match consume(&db, &url).await {
                Ok(()) => tracing::warn!("rabbitmq consumer stream ended, reconnecting"),
                Err(e) => tracing::warn!(error = %e, "rabbitmq consumer error, reconnecting in 10s"),
            }
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    });
}

async fn consume(db: &DbHandle, url: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = Connection::connect(url, ConnectionProperties::default()).await?;
    let channel = conn.create_channel().await?;
    channel
        .exchange_declare(
            EXCHANGE.into(),
            ExchangeKind::Topic,
            ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;
    channel
        .queue_declare(
            QUEUE.into(),
            QueueDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;
    channel
        .queue_bind(
            QUEUE.into(),
            EXCHANGE.into(),
            "#".into(),
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;

    tracing::info!("rabbitmq consumer listening on {QUEUE}");
    let mut consumer = channel
        .basic_consume(
            QUEUE.into(),
            "stockinvoice-mongo-mirror".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    while let Some(delivery) = consumer.next().await {
        let delivery = match delivery {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(error = %e, "rabbitmq delivery error");
                continue;
            }
        };
        let payload: serde_json::Value =
            serde_json::from_slice(&delivery.data).unwrap_or(serde_json::Value::Null);
        let routing_key = delivery.routing_key.to_string();
        let event_type = payload
            .get("event_type")
            .and_then(|v| v.as_str())
            .unwrap_or(&routing_key)
            .to_string();
        let log = EventLog {
            id: crate::util::new_id(),
            routing_key: routing_key.clone(),
            event_type,
            payload,
            received_at: now_iso(),
        };
        if let Err(e) = db.event_log().insert_one(&log).await {
            tracing::warn!(error = %e, "failed to mirror event into MongoDB");
        }
        let _ = channel
            .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
            .await;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Broker dashboard
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct QueueRow {
    pub name: String,
    pub messages: i64,
    pub consumers: i64,
    pub state: String,
}

#[derive(Debug, Clone, Default)]
pub struct BrokerInfo {
    pub configured: bool,
    pub mgmt_url: String,
    pub online: bool,
    pub version: String,
    pub node: String,
    pub queues: Vec<QueueRow>,
    pub total_messages: i64,
    pub error: Option<String>,
}

/// Derive the management API endpoint and credentials from `RABBITMQ_URL`
/// (or use `RABBITMQ_MGMT_URL` when set).
pub fn management_endpoint(cfg: &Config) -> Option<(String, Option<(String, String)>)> {
    let raw = cfg
        .rabbit_mgmt_url
        .clone()
        .or_else(|| cfg.rabbit_url.clone())?;
    let without_scheme = raw.splitn(2, "://").nth(1).unwrap_or(&raw);
    let (authority, _path) = match without_scheme.split_once('/') {
        Some((a, p)) => (a, Some(p)),
        None => (without_scheme, None),
    };
    let (creds, hostport) = match authority.rsplit_once('@') {
        Some((c, h)) => {
            let mut it = c.splitn(2, ':');
            let user = crate::auth::decode_public(it.next().unwrap_or("guest"));
            let pass = crate::auth::decode_public(it.next().unwrap_or("guest"));
            (Some((user, pass)), h)
        }
        None => (None, authority),
    };
    let host = hostport.split(':').next().unwrap_or("localhost");
    let port = hostport.split(':').nth(1).unwrap_or("15672");
    let mgmt_port = if port == "5672" || port == "5671" {
        "15672"
    } else if cfg.rabbit_mgmt_url.is_some() {
        port
    } else {
        "15672"
    };
    Some((format!("http://{host}:{mgmt_port}"), creds))
}

pub async fn broker_info(cfg: &Config) -> BrokerInfo {
    let Some((base, creds)) = management_endpoint(cfg) else {
        return BrokerInfo::default();
    };
    let mut info = BrokerInfo {
        configured: true,
        mgmt_url: base.clone(),
        ..Default::default()
    };

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            info.error = Some(e.to_string());
            return info;
        }
    };

    let mut req = client.get(format!("{base}/api/overview"));
    if let Some((u, p)) = creds.clone() {
        req = req.basic_auth(u, Some(p));
    }
    match req.send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(v) => {
                info.online = true;
                info.version = v
                    .get("rabbitmq_version")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                info.node = v
                    .get("node")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                info.total_messages = v
                    .pointer("/queue_totals/messages")
                    .and_then(|x| x.as_i64())
                    .unwrap_or(0);
            }
            Err(e) => info.error = Some(format!("invalid overview response: {e}")),
        },
        Ok(resp) => {
            info.error = Some(format!("management API returned HTTP {}", resp.status()));
        }
        Err(e) => {
            info.error = Some(e.to_string());
            return info;
        }
    }

    if info.online {
        let mut req = client.get(format!("{base}/api/queues"));
        if let Some((u, p)) = creds {
            req = req.basic_auth(u, Some(p));
        }
        if let Ok(resp) = req.send().await {
            if let Ok(list) = resp.json::<Vec<serde_json::Value>>().await {
                info.queues = list
                    .iter()
                    .map(|q| QueueRow {
                        name: q
                            .get("name")
                            .and_then(|x| x.as_str())
                            .unwrap_or("?")
                            .to_string(),
                        messages: q.get("messages").and_then(|x| x.as_i64()).unwrap_or(0),
                        consumers: q.get("consumers").and_then(|x| x.as_i64()).unwrap_or(0),
                        state: q
                            .get("state")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                    })
                    .collect();
                info.queues.sort_by(|a, b| a.name.cmp(&b.name));
            }
        }
    }

    info
}

/// Format an event time for display (kept here so the log view stays tidy).
pub fn event_age(received_at: &str) -> String {
    match parse_iso(received_at) {
        Some(t) => {
            let mins = (chrono::Utc::now() - t).num_minutes();
            if mins < 1 {
                "now".into()
            } else if mins < 60 {
                format!("{mins}m")
            } else if mins < 1440 {
                format!("{}h", mins / 60)
            } else {
                format!("{}d", mins / 1440)
            }
        }
        None => received_at.to_string(),
    }
}
