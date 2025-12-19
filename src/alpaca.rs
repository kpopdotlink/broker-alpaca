//! Alpaca Markets API Client
//!
//! Implements Alpaca's Trading API with API Key authentication.
//! Documentation: https://docs.alpaca.markets/

use crate::http::{HttpMethod, HttpRequest, execute};
use chrono::{DateTime, Utc};
use models::order::{Order, OrderRequest, OrderSide, OrderStatus, OrderType};
use models::portfolio::{AccountBalance, AccountSummary, Position};
use serde::Deserialize;
use std::collections::HashMap;

const LIVE_API_URL: &str = "https://api.alpaca.markets";
const PAPER_API_URL: &str = "https://paper-api.alpaca.markets";

pub struct AlpacaClient {
    api_key: String,
    api_secret: String,
    base_url: String,
    is_paper: bool,
}

impl AlpacaClient {
    pub fn new(api_key: String, api_secret: String, is_paper: bool) -> Self {
        let base_url = if is_paper { PAPER_API_URL } else { LIVE_API_URL };
        Self {
            api_key,
            api_secret,
            base_url: base_url.to_string(),
            is_paper,
        }
    }

    fn default_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("Accept".to_string(), "application/json".to_string());
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("APCA-API-KEY-ID".to_string(), self.api_key.clone());
        headers.insert("APCA-API-SECRET-KEY".to_string(), self.api_secret.clone());
        headers
    }

    fn api_get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);

        let response = execute(HttpRequest {
            method: HttpMethod::Get,
            url,
            headers: self.default_headers(),
            body: None,
            timeout_ms: 30000,
        });

        if !response.is_success() {
            return Err(format!(
                "API error {}: {}",
                response.status,
                response.error.unwrap_or(response.body)
            ));
        }

        response.json::<T>()
    }

    fn api_post<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);

        let body_str = serde_json::to_string(body)
            .map_err(|e| e.to_string())?;

        let response = execute(HttpRequest {
            method: HttpMethod::Post,
            url,
            headers: self.default_headers(),
            body: Some(body_str),
            timeout_ms: 30000,
        });

        if !response.is_success() {
            return Err(format!(
                "API error {}: {}",
                response.status,
                response.error.unwrap_or(response.body)
            ));
        }

        response.json::<T>()
    }

    fn api_delete(&self, path: &str) -> Result<(), String> {
        let url = format!("{}{}", self.base_url, path);

        let response = execute(HttpRequest {
            method: HttpMethod::Delete,
            url,
            headers: self.default_headers(),
            body: None,
            timeout_ms: 30000,
        });

        if !response.is_success() {
            return Err(format!(
                "API error {}: {}",
                response.status,
                response.error.unwrap_or(response.body)
            ));
        }

        Ok(())
    }

    /// Get account information
    pub fn get_account(&self) -> Result<AccountSummary, String> {
        #[derive(Deserialize)]
        struct AlpacaAccount {
            id: String,
            account_number: String,
            status: String,
            currency: String,
            cash: String,
            portfolio_value: String,
            buying_power: String,
            equity: String,
            last_equity: String,
            daytrade_count: Option<i32>,
            pattern_day_trader: Option<bool>,
        }

        let account: AlpacaAccount = self.api_get("/v2/account")?;

        let parse_amount = |s: &str| -> f64 {
            s.parse::<f64>().unwrap_or(0.0)
        };

        let positions = self.get_positions().unwrap_or_default();

        Ok(AccountSummary {
            id: account.account_number.clone(),
            name: format!("Alpaca {}", if self.is_paper { "Paper" } else { "Live" }),
            broker_id: "broker-alpaca".to_string(),
            is_paper: self.is_paper,
            balance: AccountBalance {
                currency: account.currency,
                total_equity: parse_amount(&account.equity),
                available_cash: parse_amount(&account.cash),
                buying_power: parse_amount(&account.buying_power),
                locked_cash: 0.0,
            },
            positions,
            updated_at: Utc::now(),
            extensions: Some({
                let mut map = HashMap::new();
                map.insert("account_id".to_string(), serde_json::Value::String(account.id));
                map.insert("status".to_string(), serde_json::Value::String(account.status));
                if let Some(pdt) = account.pattern_day_trader {
                    map.insert("pattern_day_trader".to_string(), serde_json::Value::Bool(pdt));
                }
                if let Some(count) = account.daytrade_count {
                    map.insert("daytrade_count".to_string(), serde_json::Value::Number(count.into()));
                }
                map
            }),
        })
    }

    /// List accounts (Alpaca has single account per API key)
    pub fn list_accounts(&self) -> Result<Vec<AccountSummary>, String> {
        let account = self.get_account()?;
        Ok(vec![account])
    }

    /// Get all positions
    pub fn get_positions(&self) -> Result<Vec<Position>, String> {
        #[derive(Deserialize)]
        struct AlpacaPosition {
            symbol: String,
            qty: String,
            avg_entry_price: String,
            current_price: String,
            market_value: String,
            unrealized_pl: String,
            unrealized_plpc: String,
            side: String,
        }

        let positions: Vec<AlpacaPosition> = self.api_get("/v2/positions")?;

        Ok(positions
            .into_iter()
            .map(|p| {
                let qty: f64 = p.qty.parse().unwrap_or(0.0);
                let multiplier = if p.side == "short" { -1.0 } else { 1.0 };

                Position {
                    symbol_id: p.symbol,
                    quantity: qty * multiplier,
                    average_price: p.avg_entry_price.parse().unwrap_or(0.0),
                    current_price: p.current_price.parse().unwrap_or(0.0),
                    unrealized_pnl: p.unrealized_pl.parse().unwrap_or(0.0),
                    unrealized_pnl_percent: p.unrealized_plpc.parse::<f64>().unwrap_or(0.0) * 100.0,
                }
            })
            .collect())
    }

    /// Submit an order
    pub fn submit_order(&self, order: &OrderRequest) -> Result<Order, String> {
        #[derive(serde::Serialize)]
        struct CreateOrderRequest {
            symbol: String,
            qty: String,
            side: String,
            #[serde(rename = "type")]
            order_type: String,
            time_in_force: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            limit_price: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            stop_price: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            client_order_id: Option<String>,
        }

        let side = match order.side {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        };

        let order_type = match order.order_type {
            OrderType::Market => "market",
            OrderType::Limit => "limit",
            OrderType::Stop => "stop",
            OrderType::StopLimit => "stop_limit",
        };

        let client_order_id = format!("KL{:016x}", rand::random::<u64>());

        let req = CreateOrderRequest {
            symbol: order.symbol_id.clone(),
            qty: order.quantity.to_string(),
            side: side.to_string(),
            order_type: order_type.to_string(),
            time_in_force: "day".to_string(),
            limit_price: order.limit_price.map(|p| p.to_string()),
            stop_price: order.stop_price.map(|p| p.to_string()),
            client_order_id: Some(client_order_id.clone()),
        };

        #[derive(Deserialize)]
        struct OrderResponse {
            id: String,
            client_order_id: String,
            status: String,
            symbol: String,
            qty: String,
            filled_qty: String,
            filled_avg_price: Option<String>,
            created_at: String,
            updated_at: String,
        }

        let resp: OrderResponse = self.api_post("/v2/orders", &req)?;

        let status = match resp.status.as_str() {
            "new" | "accepted" | "pending_new" => OrderStatus::Submitted,
            "partially_filled" => OrderStatus::PartiallyFilled,
            "filled" => OrderStatus::Filled,
            "canceled" | "expired" | "rejected" => OrderStatus::Canceled,
            _ => OrderStatus::Submitted,
        };

        let created_at = DateTime::parse_from_rfc3339(&resp.created_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let updated_at = DateTime::parse_from_rfc3339(&resp.updated_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(Order {
            id: resp.id.clone(),
            request: order.clone(),
            status,
            created_at,
            updated_at,
            filled_quantity: resp.filled_qty.parse().unwrap_or(0.0),
            average_filled_price: resp.filled_avg_price.and_then(|p| p.parse().ok()),
            extensions: Some({
                let mut map = HashMap::new();
                map.insert("client_order_id".to_string(),
                    serde_json::Value::String(resp.client_order_id));
                map.insert("alpaca_status".to_string(),
                    serde_json::Value::String(resp.status));
                map
            }),
            persona_id: order.persona_id.clone(),
        })
    }

    /// Cancel an order
    pub fn cancel_order(&self, order_id: &str) -> Result<(), String> {
        self.api_delete(&format!("/v2/orders/{}", order_id))
    }

    /// Get order by ID
    pub fn get_order(&self, order_id: &str) -> Result<Order, String> {
        #[derive(Deserialize)]
        struct OrderResponse {
            id: String,
            client_order_id: String,
            status: String,
            symbol: String,
            qty: String,
            side: String,
            #[serde(rename = "type")]
            order_type: String,
            filled_qty: String,
            filled_avg_price: Option<String>,
            limit_price: Option<String>,
            stop_price: Option<String>,
            created_at: String,
            updated_at: String,
        }

        let resp: OrderResponse = self.api_get(&format!("/v2/orders/{}", order_id))?;

        let side = match resp.side.as_str() {
            "buy" => OrderSide::Buy,
            _ => OrderSide::Sell,
        };

        let order_type = match resp.order_type.as_str() {
            "market" => OrderType::Market,
            "limit" => OrderType::Limit,
            "stop" => OrderType::Stop,
            "stop_limit" => OrderType::StopLimit,
            _ => OrderType::Market,
        };

        let status = match resp.status.as_str() {
            "new" | "accepted" | "pending_new" => OrderStatus::Submitted,
            "partially_filled" => OrderStatus::PartiallyFilled,
            "filled" => OrderStatus::Filled,
            "canceled" | "expired" | "rejected" => OrderStatus::Canceled,
            _ => OrderStatus::Submitted,
        };

        let created_at = DateTime::parse_from_rfc3339(&resp.created_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let updated_at = DateTime::parse_from_rfc3339(&resp.updated_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(Order {
            id: resp.id.clone(),
            request: OrderRequest {
                symbol_id: resp.symbol,
                quantity: resp.qty.parse().unwrap_or(0.0),
                side,
                order_type,
                limit_price: resp.limit_price.and_then(|p| p.parse().ok()),
                stop_price: resp.stop_price.and_then(|p| p.parse().ok()),
                reference_price: None,
                time_in_force: None,
                extensions: None,
                persona_id: String::new(),
            },
            status,
            created_at,
            updated_at,
            filled_quantity: resp.filled_qty.parse().unwrap_or(0.0),
            average_filled_price: resp.filled_avg_price.and_then(|p| p.parse().ok()),
            extensions: Some({
                let mut map = HashMap::new();
                map.insert("client_order_id".to_string(),
                    serde_json::Value::String(resp.client_order_id));
                map.insert("alpaca_status".to_string(),
                    serde_json::Value::String(resp.status));
                map
            }),
            persona_id: String::new(),
        })
    }
}
