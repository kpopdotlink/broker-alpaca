//! HTTP client wrapper for host function calls
//!
//! This module provides HTTP functionality through WASM host functions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Host function imports
extern "C" {
    fn http_request(ptr: i32, len: i32) -> u64;
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

#[derive(Serialize)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub timeout_ms: u32,
}

#[derive(Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub error: Option<String>,
}

impl HttpResponse {
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, String> {
        serde_json::from_str(&self.body)
            .map_err(|e| format!("JSON parse error: {} - body: {}", e, &self.body[..self.body.len().min(200)]))
    }
}

/// Execute an HTTP request through the host
pub fn execute(request: HttpRequest) -> HttpResponse {
    let req_json = serde_json::to_string(&request).expect("Failed to serialize request");
    let req_bytes = req_json.as_bytes();

    let ptr = req_bytes.as_ptr() as i32;
    let len = req_bytes.len() as i32;

    let result = unsafe { http_request(ptr, len) };

    let res_ptr = (result >> 32) as i32;
    let res_len = (result & 0xFFFFFFFF) as i32;

    let response_slice = unsafe {
        std::slice::from_raw_parts(res_ptr as *const u8, res_len as usize)
    };

    serde_json::from_slice(response_slice).unwrap_or_else(|e| HttpResponse {
        status: 0,
        headers: HashMap::new(),
        body: String::new(),
        error: Some(format!("Failed to parse response: {}", e)),
    })
}
