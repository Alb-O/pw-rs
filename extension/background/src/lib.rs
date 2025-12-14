use std::rc::Rc;

use js_sys::{Array, Object, Reflect};
use serde::{Deserialize, Serialize};
use serde_json::json;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};

use std::cell::RefCell;

const RELAY_URL: &str = "ws://127.0.0.1:19988/extension";
const LOG_LIMIT: usize = 40;

thread_local! {
    static LOG: RefCell<Vec<String>> = RefCell::new(Vec::new());
}

#[derive(Debug, Deserialize)]
struct RelayCommand {
    id: Option<u64>,
    method: String,
    params: ForwardParams,
}

#[derive(Debug, Deserialize)]
struct ForwardParams {
    method: String,
    #[serde(default)]
    params: serde_json::Value,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct ForwardEvent<'a> {
    method: &'a str,
    params: serde_json::Value,
}

#[wasm_bindgen(start)]
pub async fn start() {
    console_error_panic_hook::set_once();
    log(&format!("pw-ext-background starting; relay={RELAY_URL}"));
    set_status("connecting", "pw-rs bridge (connecting)", [160, 160, 160, 255]);

    if let Err(err) = init().await {
        let msg = format!("pw-rs bridge error: {err:?}");
        set_status("error", &msg, [200, 40, 40, 255]);
        log(&format!("init failed: {:?}", err));
    }
}

async fn init() -> Result<(), JsValue> {
    let tab_id = find_active_tab().await?;
    attach_debugger(tab_id).await?;

    let ws = WebSocket::new(RELAY_URL)?;
    let ws_rc = Rc::new(ws);

    // Handle incoming messages (commands from relay)
    {
        let ws_inner = ws_rc.clone();
        let onmessage_callback = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
                if let Err(err) = handle_ws_message(event, tab_id, ws_inner.clone()) {
                    let msg = format!("ws message error: {:?}", err);
                    log(&msg);
                    push_log(&msg);
                }

        });
        ws_rc.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();
    }

    // Log opens/errors/closes
    {
        let onopen = Closure::<dyn FnMut()>::new(|| {
            let msg = "relay websocket connected";
            log(msg);
            set_status("connected", "pw-rs bridge connected", [30, 170, 80, 255]);
            push_log(msg);
        });
        ws_rc.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        onopen.forget();

        let onerror = Closure::<dyn FnMut(ErrorEvent)>::new(|e: ErrorEvent| {
            let msg = format!("relay websocket error: {}", e.message());
            log(&msg);
            set_status("error", &msg, [200, 40, 40, 255]);
            push_log(&msg);
        });
        ws_rc.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();

        let onclose = Closure::<dyn FnMut(CloseEvent)>::new(|e: CloseEvent| {
            let msg = format!("relay websocket closed code={} reason={}", e.code(), e.reason());
            log(&msg);
            set_status("disconnected", &msg, [120, 120, 120, 255]);
            push_log(&msg);
        });
        ws_rc.set_onclose(Some(onclose.as_ref().unchecked_ref()));
        onclose.forget();
    }

    // Forward debugger events to relay
    {
        let ws_inner = ws_rc.clone();
        let on_event = Closure::<dyn FnMut(JsValue, JsValue, JsValue)>::new(
            move |_source: JsValue, method: JsValue, params: JsValue| {
                let method_str = method.as_string().unwrap_or_default();
                let session_id = Reflect::get(&params, &JsValue::from_str("sessionId"))
                    .ok()
                    .and_then(|v| v.as_string());

                let payload = json!({
                    "method": method_str,
                    "params": serde_wasm_bindgen::from_value(params.clone()).unwrap_or(json!({})),
                    "sessionId": session_id,
                });

                let msg = ForwardEvent {
                    method: "forwardCDPEvent",
                    params: payload,
                };
                if let Err(err) = send_json(&ws_inner, &msg) {
                    log(&format!("failed to send event: {:?}", err));
                }
            },
        );
        debugger_on_event_add_listener(&on_event);
        on_event.forget();
    }

    {
        let ws_inner = ws_rc.clone();
        let on_detach = Closure::<dyn FnMut(JsValue, JsValue)>::new(move |_source: JsValue, reason: JsValue| {
            let reason_str = reason.as_string().unwrap_or_default();
            let msg = ForwardEvent {
                method: "forwardCDPEvent",
                params: json!({
                    "method": "Target.detachedFromTarget",
                    "params": json!({ "reason": reason_str })
                }),
            };
            let _ = send_json(&ws_inner, &msg);
        });
        debugger_on_detach_add_listener(&on_detach);
        on_detach.forget();
    }

    Ok(())
}

fn handle_ws_message(event: MessageEvent, tab_id: i32, ws: Rc<WebSocket>) -> Result<(), JsValue> {
    let text = match event.data().as_string() {
        Some(t) => t,
        None => return Ok(()),
    };

    let cmd: RelayCommand = serde_json::from_str(&text)
        .map_err(|e| JsValue::from_str(&format!("failed to parse command: {e}")))?;

    if cmd.method != "forwardCDPCommand" {
        return Ok(());
    }

    let target = build_debuggee(tab_id, cmd.params.session_id.as_deref())?;
    let params_js = serde_wasm_bindgen::to_value(&cmd.params.params)?;

    let ws_ok = ws.clone();
    let ws_err = ws.clone();
    let cmd_id = cmd.id;
    let session_id_ok = cmd.params.session_id.clone();
    let session_id_err = cmd.params.session_id.clone();
    let method = cmd.params.method.clone();

    let future = async move {
        let result = match JsFuture::from(debugger_send_command(&target, &method, &params_js)).await {
            Ok(val) => serde_wasm_bindgen::from_value(val).unwrap_or(json!({})),
            Err(err) => {
                let msg = format!("sendCommand {} failed: {}", method, stringify_js_error(err));
                push_log(&msg);
                return Err(JsValue::from_str(&msg));
            }
        };

        let response = json!({
            "id": cmd_id,
            "sessionId": session_id_ok,
            "result": result,
        });

        send_raw_json(&ws_ok, &response)
    };

    wasm_bindgen_futures::spawn_local(async move {
        if let Err(err) = future.await {
            let resp = json!({
                "id": cmd_id,
                "sessionId": session_id_err,
                "error": {"message": stringify_js_error(err)},
            });
            let _ = send_raw_json(&ws_err, &resp);
        }
    });

    Ok(())
}

async fn find_active_tab() -> Result<i32, JsValue> {
    let query = serde_wasm_bindgen::to_value(&json!({ "active": true, "currentWindow": true }))?;
    let tabs_val = JsFuture::from(tabs_query(&query)).await?;
    let tabs = js_sys::Array::from(&tabs_val);
    let tab = tabs.get(0);
    let id = Reflect::get(&tab, &JsValue::from_str("id"))?
        .as_f64()
        .ok_or_else(|| JsValue::from_str("active tab id missing"))? as i32;
    Ok(id)
}

async fn attach_debugger(tab_id: i32) -> Result<(), JsValue> {
    let target = build_tab_object(tab_id)?;
    JsFuture::from(debugger_attach(&target, "1.3")).await?;
    Ok(())
}

fn build_tab_object(tab_id: i32) -> Result<JsValue, JsValue> {
    let target = Object::new();
    Reflect::set(&target, &JsValue::from_str("tabId"), &JsValue::from_f64(tab_id as f64))?;
    Ok(target.into())
}

fn build_debuggee(tab_id: i32, session_id: Option<&str>) -> Result<JsValue, JsValue> {
    let target = Object::new();
    Reflect::set(&target, &JsValue::from_str("tabId"), &JsValue::from_f64(tab_id as f64))?;
    if let Some(session) = session_id {
        Reflect::set(&target, &JsValue::from_str("sessionId"), &JsValue::from_str(session))?;
    }
    Ok(target.into())
}

fn send_json<T: Serialize>(ws: &WebSocket, value: &T) -> Result<(), JsValue> {
    let json = serde_json::to_string(value).map_err(|e| JsValue::from_str(&e.to_string()))?;
    ws.send_with_str(&json)
}

fn send_raw_json(ws: &WebSocket, value: &serde_json::Value) -> Result<(), JsValue> {
    let json = serde_json::to_string(value).map_err(|e| JsValue::from_str(&e.to_string()))?;
    ws.send_with_str(&json)
}

fn stringify_js_error(err: JsValue) -> String {
    if let Some(s) = err.as_string() {
        return s;
    }
    if let Ok(s) = js_sys::JSON::stringify(&err) {
        if let Some(s) = s.as_string() {
            return s;
        }
    }
    format!("{:?}", err)
}

fn log(msg: &str) {
    web_sys::console::log_1(&JsValue::from_str(msg));
}

fn set_status(status: &str, title: &str, rgba: [u8; 4]) {
    set_badge(status_text(status), title, rgba);
    persist_state(status, title);
    send_state_message(status, title);
}

fn status_text(status: &str) -> &str {
    match status {
        "connected" => "ON",
        "error" => "ERR",
        "disconnected" => "OFF",
        _ => "…",
    }
}

fn set_badge(text: &str, title: &str, rgba: [u8; 4]) {
    let color = Array::new();
    color.push(&JsValue::from_f64(rgba[0] as f64));
    color.push(&JsValue::from_f64(rgba[1] as f64));
    color.push(&JsValue::from_f64(rgba[2] as f64));
    color.push(&JsValue::from_f64(rgba[3] as f64));

    let text_obj = Object::new();
    let _ = Reflect::set(&text_obj, &JsValue::from_str("text"), &JsValue::from_str(text));
    action_set_badge_text(&text_obj);

    let color_obj = Object::new();
    let _ = Reflect::set(&color_obj, &JsValue::from_str("color"), &color);
    action_set_badge_background_color(&color_obj);

    let title_obj = Object::new();
    let _ = Reflect::set(&title_obj, &JsValue::from_str("title"), &JsValue::from_str(title));
    action_set_title(&title_obj);
}

fn push_log(line: &str) {
    LOG.with(|log| {
        let mut vec = log.borrow_mut();
        vec.push(line.to_string());
        if vec.len() > LOG_LIMIT {
            let drop = vec.len() - LOG_LIMIT;
            vec.drain(0..drop);
        }
        persist_log(&vec);
        send_log_message(&vec);
    });
}

fn persist_state(status: &str, message: &str) {
    let obj = Object::new();
    let state = Object::new();
    let _ = Reflect::set(&state, &JsValue::from_str("status"), &JsValue::from_str(status));
    let _ = Reflect::set(&state, &JsValue::from_str("message"), &JsValue::from_str(message));
    let _ = Reflect::set(
        &obj,
        &JsValue::from_str("pw_bridge_state"),
        &state,
    );
    let _ = storage_local_set(&obj);
}

fn persist_log(lines: &[String]) {
    let array = Array::new();
    for line in lines {
        array.push(&JsValue::from_str(line));
    }
    let obj = Object::new();
    let _ = Reflect::set(&obj, &JsValue::from_str("pw_bridge_log"), &array);
    let _ = storage_local_set(&obj);
}

fn send_state_message(status: &str, message: &str) {
    let payload = Object::new();
    let _ = Reflect::set(&payload, &JsValue::from_str("type"), &JsValue::from_str("pw-bridge-state"));
    let _ = Reflect::set(&payload, &JsValue::from_str("status"), &JsValue::from_str(status));
    let _ = Reflect::set(&payload, &JsValue::from_str("message"), &JsValue::from_str(message));
    let _ = runtime_send_message(&payload);
}

fn send_log_message(lines: &[String]) {
    let array = Array::new();
    for line in lines {
        array.push(&JsValue::from_str(line));
    }
    let payload = Object::new();
    let _ = Reflect::set(&payload, &JsValue::from_str("type"), &JsValue::from_str("pw-bridge-log"));
    let _ = Reflect::set(&payload, &JsValue::from_str("lines"), &array);
    let _ = runtime_send_message(&payload);
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["chrome", "tabs"], js_name = query)]
    fn tabs_query(query: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = ["chrome", "debugger"], js_name = attach)]
    fn debugger_attach(target: &JsValue, version: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = ["chrome", "debugger"], js_name = sendCommand)]
    fn debugger_send_command(target: &JsValue, method: &str, params: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = ["chrome", "debugger", "onEvent"], js_name = addListener)]
    fn debugger_on_event_add_listener(cb: &Closure<dyn FnMut(JsValue, JsValue, JsValue)>);

    #[wasm_bindgen(js_namespace = ["chrome", "debugger", "onDetach"], js_name = addListener)]
    fn debugger_on_detach_add_listener(cb: &Closure<dyn FnMut(JsValue, JsValue)>);

    #[wasm_bindgen(js_namespace = ["chrome", "action"], js_name = setBadgeText)]
    fn action_set_badge_text(details: &JsValue);

    #[wasm_bindgen(js_namespace = ["chrome", "action"], js_name = setBadgeBackgroundColor)]
    fn action_set_badge_background_color(details: &JsValue);

    #[wasm_bindgen(js_namespace = ["chrome", "action"], js_name = setTitle)]
    fn action_set_title(details: &JsValue);

    #[wasm_bindgen(js_namespace = ["chrome", "runtime"], js_name = sendMessage)]
    fn runtime_send_message(message: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = ["chrome", "storage", "local"], js_name = set)]
    fn storage_local_set(items: &JsValue) -> js_sys::Promise;
}
