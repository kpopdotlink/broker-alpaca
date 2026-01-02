//! Alpaca Markets Broker Plugin for KL Investment
//!
//! This plugin integrates with Alpaca's Trading API to provide:
//! - Commission-free stock and ETF trading
//! - Paper trading support
//! - Real-time account and position data
//!
//! ## Authentication
//! Alpaca uses API Key + Secret authentication via HTTP headers.
//! - APCA-API-KEY-ID: Your API Key
//! - APCA-API-SECRET-KEY: Your API Secret

// Allow dead_code for structs/fields prepared for future API integration
#![allow(dead_code)]

mod alpaca;
mod http;

use chrono::Utc;
use std::collections::HashMap;
use std::slice;
use std::sync::Mutex;

use alpaca::AlpacaClient;
use models::order::{Order, OrderStatus};
use models::portfolio::{AccountBalance, AccountSummary};
use plugin_api::{
    GetAccountsRequest, GetAccountsResponse, GetPositionsRequest, GetPositionsResponse,
    SubmitOrderRequest, SubmitOrderResponse,
};

// --- State Management ---

struct BrokerState {
    client: Option<AlpacaClient>,
    orders: HashMap<String, Order>,
}

impl BrokerState {
    fn new() -> Self {
        Self {
            client: None,
            orders: HashMap::new(),
        }
    }
}

lazy_static::lazy_static! {
    static ref STATE: Mutex<BrokerState> = Mutex::new(BrokerState::new());
}

// --- WASM Exports ---

/// Memory allocation for host communication
#[no_mangle]
pub extern "C" fn alloc(len: i32) -> i32 {
    let mut buf: Vec<u8> = Vec::with_capacity(len as usize);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr as usize as i32
}

/// Initialize plugin with configuration
#[no_mangle]
pub extern "C" fn initialize(ptr: i32, len: i32) -> u64 {
    let config_json: serde_json::Value = parse_request(ptr, len);

    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());

    // Parse configuration
    let api_key = config_json
        .get("api_key")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let api_secret = config_json
        .get("api_secret")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let is_paper = config_json
        .get("is_paper")
        .and_then(|v| v.as_bool())
        .unwrap_or(true); // Default to paper trading for safety

    // Validate configuration
    match (api_key, api_secret) {
        (Some(key), Some(secret)) if !key.is_empty() && !secret.is_empty() => {
            let client = AlpacaClient::new(key, secret, is_paper);
            state.client = Some(client);

            serialize_response(&serde_json::json!({
                "success": true,
                "message": format!("Alpaca plugin initialized ({})", if is_paper { "paper" } else { "live" })
            }))
        }
        _ => serialize_response(&serde_json::json!({
            "success": false,
            "error": "Missing required configuration: api_key and api_secret",
            "requires_auth": true
        })),
    }
}

/// Get available accounts
#[no_mangle]
pub extern "C" fn get_accounts(ptr: i32, len: i32) -> u64 {
    let _req: GetAccountsRequest = parse_request(ptr, len);

    let state = STATE.lock().unwrap_or_else(|e| e.into_inner());

    let client = match state.client.as_ref() {
        Some(c) => c,
        None => {
            return serialize_response(&GetAccountsResponse {
                accounts: vec![create_error_account(
                    "Plugin not initialized. Provide api_key and api_secret.",
                )],
            });
        }
    };

    match client.list_accounts() {
        Ok(accounts) => serialize_response(&GetAccountsResponse { accounts }),
        Err(e) => {
            eprintln!("[broker-alpaca] Failed to fetch accounts: {}", e);
            serialize_response(&GetAccountsResponse {
                accounts: vec![create_error_account(&e)],
            })
        }
    }
}

/// Get positions for an account
#[no_mangle]
pub extern "C" fn get_positions(ptr: i32, len: i32) -> u64 {
    let _req: GetPositionsRequest = parse_request(ptr, len);

    let state = STATE.lock().unwrap_or_else(|e| e.into_inner());

    let client = match state.client.as_ref() {
        Some(c) => c,
        None => {
            return serialize_response(&GetPositionsResponse { positions: vec![] });
        }
    };

    match client.get_positions() {
        Ok(positions) => serialize_response(&GetPositionsResponse { positions }),
        Err(e) => {
            eprintln!("[broker-alpaca] Failed to fetch positions: {}", e);
            serialize_response(&GetPositionsResponse { positions: vec![] })
        }
    }
}

/// Submit an order
#[no_mangle]
pub extern "C" fn submit_order(ptr: i32, len: i32) -> u64 {
    let req: SubmitOrderRequest = parse_request(ptr, len);
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());

    let client = match state.client.as_ref() {
        Some(c) => c,
        None => {
            return serialize_response(&SubmitOrderResponse {
                order: create_error_order(&req, "Plugin not initialized"),
            });
        }
    };

    match client.submit_order(&req.order) {
        Ok(mut order) => {
            let order_id = order.id.clone();
            if order.persona_id.is_empty() {
                order.persona_id = req.order.persona_id.clone();
            }
            state.orders.insert(order_id, order.clone());

            serialize_response(&SubmitOrderResponse { order })
        }
        Err(e) => {
            eprintln!("[broker-alpaca] Order failed: {}", e);
            serialize_response(&SubmitOrderResponse {
                order: create_error_order(&req, &e),
            })
        }
    }
}

/// Cancel an order
#[no_mangle]
pub extern "C" fn cancel_order(ptr: i32, len: i32) -> u64 {
    #[derive(serde::Deserialize)]
    struct CancelOrderRequest {
        order_id: String,
    }

    let req: CancelOrderRequest = parse_request(ptr, len);
    let state = STATE.lock().unwrap_or_else(|e| e.into_inner());

    let client = match state.client.as_ref() {
        Some(c) => c,
        None => {
            return serialize_response(&serde_json::json!({
                "success": false,
                "error": "Plugin not initialized"
            }));
        }
    };

    match client.cancel_order(&req.order_id) {
        Ok(()) => serialize_response(&serde_json::json!({
            "success": true,
            "order_id": req.order_id
        })),
        Err(e) => serialize_response(&serde_json::json!({
            "success": false,
            "error": e
        })),
    }
}

// --- Helper Functions ---

fn parse_request<T: serde::de::DeserializeOwned>(ptr: i32, len: i32) -> T {
    let slice = unsafe { slice::from_raw_parts(ptr as *const u8, len as usize) };
    serde_json::from_slice(slice).expect("Failed to parse request")
}

fn serialize_response<T: serde::Serialize>(response: &T) -> u64 {
    let res_bytes = serde_json::to_vec(response).expect("Failed to serialize response");

    let out_len = res_bytes.len() as i32;
    let out_ptr = alloc(out_len);

    unsafe {
        std::ptr::copy_nonoverlapping(res_bytes.as_ptr(), out_ptr as *mut u8, out_len as usize);
    }

    ((out_ptr as u64) << 32) | (out_len as u64)
}

fn create_error_account(error: &str) -> AccountSummary {
    AccountSummary {
        id: "error".to_string(),
        name: format!("Error: {}", error),
        broker_id: "broker-alpaca".to_string(),
        is_paper: true,
        balance: AccountBalance {
            currency: "USD".to_string(),
            total_equity: 0.0,
            available_cash: 0.0,
            buying_power: 0.0,
            locked_cash: 0.0,
        },
        positions: vec![],
        updated_at: Utc::now(),
        extensions: None,
    }
}

fn create_error_order(req: &SubmitOrderRequest, error: &str) -> Order {
    Order {
        id: format!("error_{}", Utc::now().timestamp_millis()),
        request: req.order.clone(),
        status: OrderStatus::Rejected,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        average_filled_price: None,
        filled_quantity: 0.0,
        extensions: Some({
            let mut map = HashMap::new();
            map.insert(
                "error".to_string(),
                serde_json::Value::String(error.to_string()),
            );
            map
        }),
        persona_id: req.order.persona_id.clone(),
    }
}
