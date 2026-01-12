//! Background service worker for the pw Cookie Export extension.
//!
//! This WASM module handles communication between the popup UI and the
//! `pw auth listen` server. It manages:
//!
//! - WebSocket connection to the CLI's auth listener
//! - Authentication via one-time token
//! - Fetching cookies from Chrome's cookies API
//! - Sending cookies to the CLI for storage
//!
//! # Message Flow
//!
//! ```text
//! Popup  <-->  Background (this module)  <-->  pw auth listen
//!   |              |                              |
//!   |--Connect---->|                              |
//!   |              |----WebSocket + Hello-------->|
//!   |              |<---Welcome/Rejected----------|
//!   |<--Status-----|                              |
//!   |              |                              |
//!   |--Export----->|                              |
//!   |              |---(fetch chrome.cookies)     |
//!   |              |----PushCookies-------------->|
//!   |              |<---Received/Error------------|
//!   |<--Result-----|                              |
//! ```

use std::cell::RefCell;

use js_sys::{Array, Object, Promise, Reflect};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};

/// Message from the popup UI to the background worker.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum PopupMessage {
    Connect { server: String, token: String },
    Export { domains: Vec<String> },
    GetStatus,
    GetCurrentDomain,
}

/// Response from the background worker to the popup UI.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BackgroundResponse {
    Status {
        connected: bool,
        authenticated: bool,
        server: Option<String>,
    },
    CurrentDomain {
        domain: Option<String>,
    },
    ExportResult {
        success: bool,
        domains_saved: usize,
        paths: Vec<String>,
        error: Option<String>,
    },
    Error {
        message: String,
    },
}

/// Server response messages (mirrors `pw_protocol::ServerMessage`).
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMessage {
    Welcome {
        version: String,
    },
    Rejected {
        reason: String,
    },
    Received {
        domains_saved: usize,
        paths: Vec<String>,
    },
    Error {
        message: String,
    },
}

/// Messages sent to the server (mirrors `pw_protocol::ExtensionMessage`).
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ExtensionMessage {
    Hello { token: String },
    PushCookies { domains: Vec<DomainCookies> },
}

#[derive(Debug, Serialize)]
struct DomainCookies {
    domain: String,
    cookies: Vec<ChromeCookie>,
}

/// Cookie as returned by the `chrome.cookies.getAll` API.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ChromeCookie {
    name: String,
    value: String,
    domain: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    expiration_date: Option<f64>,
    http_only: bool,
    secure: bool,
    same_site: String,
    host_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    store_id: Option<String>,
}

impl ChromeCookie {
    fn from_js(val: &JsValue) -> Option<Self> {
        Some(Self {
            name: get_string(val, "name")?,
            value: get_string(val, "value")?,
            domain: get_string(val, "domain")?,
            path: get_string(val, "path").unwrap_or_default(),
            expiration_date: get_f64(val, "expirationDate"),
            http_only: get_bool(val, "httpOnly"),
            secure: get_bool(val, "secure"),
            same_site: get_string(val, "sameSite").unwrap_or_default(),
            host_only: get_bool(val, "hostOnly"),
            store_id: get_string(val, "storeId"),
        })
    }
}

fn get_string(obj: &JsValue, key: &str) -> Option<String> {
    Reflect::get(obj, &key.into()).ok()?.as_string()
}

fn get_f64(obj: &JsValue, key: &str) -> Option<f64> {
    Reflect::get(obj, &key.into()).ok()?.as_f64()
}

fn get_bool(obj: &JsValue, key: &str) -> bool {
    Reflect::get(obj, &key.into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

thread_local! {
    static STATE: RefCell<ConnectionState> = const { RefCell::new(ConnectionState::new()) };
}

struct ConnectionState {
    ws: Option<WebSocket>,
    server: Option<String>,
    authenticated: bool,
}

impl ConnectionState {
    const fn new() -> Self {
        Self {
            ws: None,
            server: None,
            authenticated: false,
        }
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();

    let listener = Closure::<dyn FnMut(JsValue, JsValue, JsValue) -> JsValue>::new(
        |message: JsValue, _sender: JsValue, send_response: JsValue| {
            let send_fn = js_sys::Function::from(send_response);

            wasm_bindgen_futures::spawn_local(async move {
                let response = handle_popup_message(message).await;
                let response_js = serde_wasm_bindgen::to_value(&response).unwrap_or(JsValue::NULL);
                let _ = send_fn.call1(&JsValue::NULL, &response_js);
            });

            JsValue::TRUE
        },
    );

    runtime_on_message_add_listener(&listener);
    listener.forget();

    log("pw Cookie Export started");
}

async fn handle_popup_message(message: JsValue) -> BackgroundResponse {
    let msg: PopupMessage = match serde_wasm_bindgen::from_value(message) {
        Ok(m) => m,
        Err(e) => {
            return BackgroundResponse::Error {
                message: format!("Invalid message: {e}"),
            };
        }
    };

    match msg {
        PopupMessage::GetStatus => get_status(),
        PopupMessage::GetCurrentDomain => get_current_domain().await,
        PopupMessage::Connect { server, token } => connect_to_server(&server, &token).await,
        PopupMessage::Export { domains } => export_cookies(domains).await,
    }
}

fn get_status() -> BackgroundResponse {
    STATE.with(|state| {
        let s = state.borrow();
        BackgroundResponse::Status {
            connected: s
                .ws
                .as_ref()
                .is_some_and(|ws| ws.ready_state() == WebSocket::OPEN),
            authenticated: s.authenticated,
            server: s.server.clone(),
        }
    })
}

async fn get_current_domain() -> BackgroundResponse {
    match get_active_tab_domain().await {
        Ok(domain) => BackgroundResponse::CurrentDomain { domain },
        Err(e) => BackgroundResponse::Error {
            message: format!("Failed to get domain: {e:?}"),
        },
    }
}

async fn get_active_tab_domain() -> Result<Option<String>, JsValue> {
    let query = serde_wasm_bindgen::to_value(&serde_json::json!({
        "active": true,
        "currentWindow": true
    }))?;

    let tabs_val = JsFuture::from(tabs_query(&query)).await?;
    let tabs = Array::from(&tabs_val);

    if tabs.length() == 0 {
        return Ok(None);
    }

    let tab = tabs.get(0);
    let url = Reflect::get(&tab, &"url".into())?
        .as_string()
        .unwrap_or_default();

    Ok(extract_domain(&url))
}

fn extract_domain(url: &str) -> Option<String> {
    let url = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let domain = url.split('/').next()?;
    let domain = domain.split(':').next()?;
    Some(domain.to_string())
}

async fn connect_to_server(server: &str, token: &str) -> BackgroundResponse {
    STATE.with(|state| {
        let mut s = state.borrow_mut();
        if let Some(ws) = s.ws.take() {
            let _ = ws.close();
        }
        s.authenticated = false;
        s.server = Some(server.to_string());
    });

    let ws = match WebSocket::new(server) {
        Ok(ws) => ws,
        Err(e) => {
            return BackgroundResponse::Error {
                message: format!("Failed to connect: {e:?}"),
            };
        }
    };

    let token = token.to_string();

    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(|event: MessageEvent| {
        if let Some(text) = event.data().as_string() {
            handle_server_message(&text);
        }
    });
    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    let ws_clone = ws.clone();
    let token_clone = token.clone();
    let onopen = Closure::<dyn FnMut()>::new(move || {
        log("Connected, sending hello");
        let hello = ExtensionMessage::Hello {
            token: token_clone.clone(),
        };
        if let Ok(json) = serde_json::to_string(&hello) {
            let _ = ws_clone.send_with_str(&json);
        }
    });
    ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    let onerror = Closure::<dyn FnMut(ErrorEvent)>::new(|e: ErrorEvent| {
        log(&format!("WebSocket error: {}", e.message()));
        STATE.with(|state| state.borrow_mut().authenticated = false);
    });
    ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    let onclose = Closure::<dyn FnMut(CloseEvent)>::new(|_| {
        log("WebSocket closed");
        STATE.with(|state| {
            let mut s = state.borrow_mut();
            s.ws = None;
            s.authenticated = false;
        });
    });
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();

    STATE.with(|state| state.borrow_mut().ws = Some(ws));

    BackgroundResponse::Status {
        connected: true,
        authenticated: false,
        server: Some(server.to_string()),
    }
}

fn handle_server_message(text: &str) {
    let msg: ServerMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            log(&format!("Invalid server message: {e}"));
            return;
        }
    };

    match msg {
        ServerMessage::Welcome { version } => {
            log(&format!("Authenticated with server v{version}"));
            STATE.with(|state| state.borrow_mut().authenticated = true);
            notify_popup(&BackgroundResponse::Status {
                connected: true,
                authenticated: true,
                server: STATE.with(|s| s.borrow().server.clone()),
            });
        }
        ServerMessage::Rejected { reason } => {
            log(&format!("Authentication rejected: {reason}"));
            STATE.with(|state| {
                let mut s = state.borrow_mut();
                s.authenticated = false;
                if let Some(ws) = s.ws.take() {
                    let _ = ws.close();
                }
            });
            notify_popup(&BackgroundResponse::Error { message: reason });
        }
        ServerMessage::Received {
            domains_saved,
            paths,
        } => {
            log(&format!("Saved {domains_saved} domain(s)"));
            notify_popup(&BackgroundResponse::ExportResult {
                success: true,
                domains_saved,
                paths,
                error: None,
            });
        }
        ServerMessage::Error { message } => {
            log(&format!("Server error: {message}"));
            notify_popup(&BackgroundResponse::Error { message });
        }
    }
}

fn notify_popup(response: &BackgroundResponse) {
    if let Ok(msg) = serde_wasm_bindgen::to_value(response) {
        let _ = runtime_send_message(&msg);
    }
}

async fn export_cookies(domains: Vec<String>) -> BackgroundResponse {
    let (ws, authenticated) = STATE.with(|state| {
        let s = state.borrow();
        (s.ws.clone(), s.authenticated)
    });

    let ws = match ws {
        Some(ws) if ws.ready_state() == WebSocket::OPEN => ws,
        _ => {
            return BackgroundResponse::Error {
                message: "Not connected to server".into(),
            };
        }
    };

    if !authenticated {
        return BackgroundResponse::Error {
            message: "Not authenticated".into(),
        };
    }

    let mut domain_cookies = Vec::new();

    for domain in &domains {
        match fetch_cookies_for_domain(domain).await {
            Ok(cookies) => {
                log(&format!("Fetched {} cookies for {domain}", cookies.len()));
                domain_cookies.push(DomainCookies {
                    domain: domain.clone(),
                    cookies,
                });
            }
            Err(e) => {
                log(&format!("Failed to fetch cookies for {domain}: {e:?}"));
            }
        }
    }

    if domain_cookies.is_empty() {
        return BackgroundResponse::Error {
            message: "No cookies found for any domain".into(),
        };
    }

    let msg = ExtensionMessage::PushCookies {
        domains: domain_cookies,
    };
    match serde_json::to_string(&msg) {
        Ok(json) => {
            if let Err(e) = ws.send_with_str(&json) {
                return BackgroundResponse::Error {
                    message: format!("Failed to send: {e:?}"),
                };
            }
        }
        Err(e) => {
            return BackgroundResponse::Error {
                message: format!("Failed to serialize: {e}"),
            };
        }
    }

    BackgroundResponse::Status {
        connected: true,
        authenticated: true,
        server: STATE.with(|s| s.borrow().server.clone()),
    }
}

async fn fetch_cookies_for_domain(domain: &str) -> Result<Vec<ChromeCookie>, JsValue> {
    let mut all_cookies = Vec::new();

    for domain_pattern in [domain.to_string(), format!(".{domain}")] {
        let query = Object::new();
        Reflect::set(
            &query,
            &"domain".into(),
            &JsValue::from_str(&domain_pattern),
        )?;

        let cookies_val = JsFuture::from(cookies_get_all(&query)).await?;
        let cookies = Array::from(&cookies_val);

        for i in 0..cookies.length() {
            let cookie = cookies.get(i);
            if let Some(c) = ChromeCookie::from_js(&cookie) {
                let is_duplicate = all_cookies.iter().any(|existing: &ChromeCookie| {
                    existing.name == c.name
                        && existing.domain == c.domain
                        && existing.path == c.path
                });
                if !is_duplicate {
                    all_cookies.push(c);
                }
            }
        }
    }

    Ok(all_cookies)
}

fn log(msg: &str) {
    web_sys::console::log_1(&format!("[pw-ext] {msg}").into());
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["chrome", "tabs"], js_name = query)]
    fn tabs_query(query: &JsValue) -> Promise;

    #[wasm_bindgen(js_namespace = ["chrome", "cookies"], js_name = getAll)]
    fn cookies_get_all(details: &JsValue) -> Promise;

    #[wasm_bindgen(js_namespace = ["chrome", "runtime", "onMessage"], js_name = addListener)]
    fn runtime_on_message_add_listener(
        callback: &Closure<dyn FnMut(JsValue, JsValue, JsValue) -> JsValue>,
    );

    #[wasm_bindgen(js_namespace = ["chrome", "runtime"], js_name = sendMessage)]
    fn runtime_send_message(message: &JsValue) -> Promise;
}
