use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use js_sys::{Array, Object, Reflect};
use serde::{Deserialize, Serialize};
use serde_json::json;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};

const RELAY_URL: &str = "ws://127.0.0.1:19988/extension";
const LOG_LIMIT: usize = 40;

thread_local! {
    static LOG: RefCell<Vec<String>> = RefCell::new(Vec::new());
    static SESSION_TO_TAB: RefCell<HashMap<String, i32>> = RefCell::new(HashMap::new());
    static WS: RefCell<Option<Rc<WebSocket>>> = RefCell::new(None);
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
    set_status(
        "connecting",
        "pw-rs bridge (connecting)",
        [160, 160, 160, 255],
    );

    // Clear any existing debugger sessions
    let _ = reset_debugger().await;

    if let Err(err) = init().await {
        set_status("error", &format!("{err:?}"), [200, 40, 40, 255]);
        push_log(&format!("init failed: {:?}", err));
    }
}

async fn reset_debugger() -> Result<(), JsValue> {
    let targets_val = JsFuture::from(debugger_get_targets()).await?;
    let targets = js_sys::Array::from(&targets_val);

    for i in 0..targets.length() {
        let target = targets.get(i);
        let attached = Reflect::get(&target, &JsValue::from_str("attached"))?
            .as_bool()
            .unwrap_or(false);
        let tab_id = Reflect::get(&target, &JsValue::from_str("tabId"))?.as_f64();

        if attached {
            if let Some(id) = tab_id {
                let debuggee = build_tab_object(id as i32)?;
                let _ = JsFuture::from(debugger_detach(&debuggee)).await;
            }
        }
    }
    Ok(())
}

async fn init() -> Result<(), JsValue> {
    let ws = WebSocket::new(RELAY_URL)?;
    let ws_rc = Rc::new(ws);
    WS.with(|w| *w.borrow_mut() = Some(ws_rc.clone()));

    // Handle incoming commands
    {
        let ws_inner = ws_rc.clone();
        let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            let ws_clone = ws_inner.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let _ = handle_message(event, ws_clone).await;
            });
        });
        ws_rc.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        onmessage.forget();
    }

    // Connection handlers
    {
        let onopen = Closure::<dyn FnMut()>::new(|| {
            set_status("connected", "pw-rs bridge connected", [30, 170, 80, 255]);
            push_log("connected");
        });
        ws_rc.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        onopen.forget();

        let onerror = Closure::<dyn FnMut(ErrorEvent)>::new(|e: ErrorEvent| {
            set_status("error", &e.message(), [200, 40, 40, 255]);
        });
        ws_rc.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();

        let onclose = Closure::<dyn FnMut(CloseEvent)>::new(|_| {
            set_status("disconnected", "relay disconnected", [120, 120, 120, 255]);
        });
        ws_rc.set_onclose(Some(onclose.as_ref().unchecked_ref()));
        onclose.forget();
    }

    // Forward debugger events to relay
    {
        let ws_inner = ws_rc.clone();
        let on_event = Closure::<dyn FnMut(JsValue, JsValue, JsValue)>::new(
            move |source: JsValue, method: JsValue, params: JsValue| {
                let method_str = method.as_string().unwrap_or_default();
                let tab_id = Reflect::get(&source, &JsValue::from_str("tabId"))
                    .ok()
                    .and_then(|v| v.as_f64())
                    .map(|v| v as i32);

                let session_id = tab_id.and_then(|tid| {
                    SESSION_TO_TAB.with(|m| {
                        m.borrow()
                            .iter()
                            .find(|(_, &t)| t == tid)
                            .map(|(s, _)| s.clone())
                    })
                });

                let msg = ForwardEvent {
                    method: "forwardCDPEvent",
                    params: json!({
                        "method": method_str,
                        "params": serde_wasm_bindgen::from_value(params).unwrap_or(json!({})),
                        "sessionId": session_id,
                    }),
                };
                let _ = send_json(&ws_inner, &msg);
            },
        );
        debugger_on_event_add_listener(&on_event);
        on_event.forget();
    }

    // Handle debugger detach
    {
        let ws_inner = ws_rc.clone();
        let on_detach = Closure::<dyn FnMut(JsValue, JsValue)>::new(move |source: JsValue, _| {
            let tab_id = Reflect::get(&source, &JsValue::from_str("tabId"))
                .ok()
                .and_then(|v| v.as_f64())
                .map(|v| v as i32);

            if let Some(tid) = tab_id {
                let session_id = SESSION_TO_TAB.with(|m| {
                    let mut map = m.borrow_mut();
                    let sid = map.iter().find(|(_, &t)| t == tid).map(|(s, _)| s.clone());
                    if let Some(ref s) = sid {
                        map.remove(s);
                    }
                    sid
                });

                if let Some(sid) = session_id {
                    let msg = ForwardEvent {
                        method: "forwardCDPEvent",
                        params: json!({
                            "method": "Target.detachedFromTarget",
                            "params": {"sessionId": sid}
                        }),
                    };
                    let _ = send_json(&ws_inner, &msg);
                }
            }
        });
        debugger_on_detach_add_listener(&on_detach);
        on_detach.forget();
    }

    Ok(())
}

async fn handle_message(event: MessageEvent, ws: Rc<WebSocket>) -> Result<(), JsValue> {
    let text = event.data().as_string().ok_or("no text")?;
    let cmd: RelayCommand =
        serde_json::from_str(&text).map_err(|e| JsValue::from_str(&e.to_string()))?;

    if cmd.method != "forwardCDPCommand" {
        return Ok(());
    }

    let method = &cmd.params.method;
    let cmd_id = cmd.id;
    let session_id = cmd.params.session_id.clone();

    // Target.createTarget -> attach to active tab
    if method == "Target.createTarget" {
        let result = attach_to_active_tab(&ws).await;
        let response = match result {
            Ok(target_id) => json!({"id": cmd_id, "result": {"targetId": target_id}}),
            Err(e) => json!({"id": cmd_id, "error": {"message": stringify_js_error(e)}}),
        };
        return send_raw_json(&ws, &response);
    }

    // Target.closeTarget -> stub
    if method == "Target.closeTarget" {
        return send_raw_json(&ws, &json!({"id": cmd_id, "result": {"success": true}}));
    }

    // Stub unsupported commands
    let unsupported = [
        "Page.setLifecycleEventsEnabled",
        "Page.addScriptToEvaluateOnNewDocument",
        "Target.setAutoAttach",
        "Emulation.setFocusEmulationEnabled",
        "Page.createIsolatedWorld",
    ];
    if unsupported.contains(&method.as_str()) {
        return send_raw_json(
            &ws,
            &json!({"id": cmd_id, "sessionId": session_id, "result": {}}),
        );
    }

    // Find tab for session
    let tab_id = session_id
        .as_ref()
        .and_then(|sid| SESSION_TO_TAB.with(|m| m.borrow().get(sid).copied()))
        .ok_or_else(|| JsValue::from_str("Unknown session"))?;

    let target = build_tab_object(tab_id)?;
    let params_js = if cmd.params.params.is_null() {
        JsValue::UNDEFINED
    } else {
        serde_wasm_bindgen::to_value(&cmd.params.params)?
    };

    let result = JsFuture::from(debugger_send_command(&target, method, &params_js)).await;

    let response = match result {
        Ok(val) => json!({
            "id": cmd_id,
            "sessionId": session_id,
            "result": serde_wasm_bindgen::from_value(val).unwrap_or(json!({})),
        }),
        Err(e) => json!({
            "id": cmd_id,
            "sessionId": session_id,
            "error": {"message": stringify_js_error(e)},
        }),
    };

    send_raw_json(&ws, &response)
}

async fn attach_to_active_tab(ws: &Rc<WebSocket>) -> Result<String, JsValue> {
    let (tab_id, tab_url, tab_title) = find_active_tab().await?;

    let target = build_tab_object(tab_id)?;
    JsFuture::from(debugger_attach(&target, "1.3")).await?;

    let info_result = JsFuture::from(debugger_send_command(
        &target,
        "Target.getTargetInfo",
        &JsValue::UNDEFINED,
    ))
    .await?;

    let target_info = Reflect::get(&info_result, &JsValue::from_str("targetInfo"))?;
    let target_id = Reflect::get(&target_info, &JsValue::from_str("targetId"))?
        .as_string()
        .unwrap_or_else(|| format!("tab-{}", tab_id));

    let session_id = format!("pw-tab-{}", tab_id);
    SESSION_TO_TAB.with(|m| m.borrow_mut().insert(session_id.clone(), tab_id));

    let event = ForwardEvent {
        method: "forwardCDPEvent",
        params: json!({
            "method": "Target.attachedToTarget",
            "params": {
                "sessionId": session_id,
                "targetInfo": serde_wasm_bindgen::from_value::<serde_json::Value>(target_info).unwrap_or(json!({
                    "targetId": target_id,
                    "type": "page",
                    "title": tab_title,
                    "url": tab_url,
                    "attached": true,
                })),
                "waitingForDebugger": false
            }
        }),
    };
    let _ = send_json(ws, &event);
    push_log(&format!("attached: {} ({})", tab_id, tab_url));

    Ok(target_id)
}

async fn find_active_tab() -> Result<(i32, String, String), JsValue> {
    let query = serde_wasm_bindgen::to_value(&json!({"active": true, "currentWindow": true}))?;
    let tabs_val = JsFuture::from(tabs_query(&query)).await?;
    let tabs = js_sys::Array::from(&tabs_val);
    let tab = tabs.get(0);

    let id = Reflect::get(&tab, &JsValue::from_str("id"))?
        .as_f64()
        .ok_or("no tab id")? as i32;
    let url = Reflect::get(&tab, &JsValue::from_str("url"))?
        .as_string()
        .unwrap_or_default();
    let title = Reflect::get(&tab, &JsValue::from_str("title"))?
        .as_string()
        .unwrap_or_default();

    Ok((id, url, title))
}

fn build_tab_object(tab_id: i32) -> Result<JsValue, JsValue> {
    let obj = Object::new();
    Reflect::set(
        &obj,
        &JsValue::from_str("tabId"),
        &JsValue::from_f64(tab_id as f64),
    )?;
    Ok(obj.into())
}

fn send_json<T: Serialize>(ws: &WebSocket, value: &T) -> Result<(), JsValue> {
    ws.send_with_str(&serde_json::to_string(value).map_err(|e| JsValue::from_str(&e.to_string()))?)
}

fn send_raw_json(ws: &WebSocket, value: &serde_json::Value) -> Result<(), JsValue> {
    ws.send_with_str(&serde_json::to_string(value).map_err(|e| JsValue::from_str(&e.to_string()))?)
}

fn stringify_js_error(err: JsValue) -> String {
    err.as_string()
        .or_else(|| js_sys::JSON::stringify(&err).ok()?.as_string())
        .unwrap_or_else(|| format!("{:?}", err))
}

fn set_status(status: &str, title: &str, rgba: [u8; 4]) {
    let text = match status {
        "connected" => "ON",
        "error" => "ERR",
        "disconnected" => "OFF",
        _ => "...",
    };

    let color = Array::new();
    for c in rgba {
        color.push(&JsValue::from_f64(c as f64));
    }

    let text_obj = Object::new();
    let _ = Reflect::set(
        &text_obj,
        &JsValue::from_str("text"),
        &JsValue::from_str(text),
    );
    action_set_badge_text(&text_obj);

    let color_obj = Object::new();
    let _ = Reflect::set(&color_obj, &JsValue::from_str("color"), &color);
    action_set_badge_background_color(&color_obj);

    let title_obj = Object::new();
    let _ = Reflect::set(
        &title_obj,
        &JsValue::from_str("title"),
        &JsValue::from_str(title),
    );
    action_set_title(&title_obj);

    persist_state(status, title);
}

fn push_log(line: &str) {
    LOG.with(|log| {
        let mut vec = log.borrow_mut();
        vec.push(line.to_string());
        if vec.len() > LOG_LIMIT {
            let excess = vec.len() - LOG_LIMIT;
            vec.drain(0..excess);
        }
        persist_log(&vec);
    });
}

fn persist_state(status: &str, message: &str) {
    let obj = Object::new();
    let state = Object::new();
    let _ = Reflect::set(
        &state,
        &JsValue::from_str("status"),
        &JsValue::from_str(status),
    );
    let _ = Reflect::set(
        &state,
        &JsValue::from_str("message"),
        &JsValue::from_str(message),
    );
    let _ = Reflect::set(&obj, &JsValue::from_str("pw_bridge_state"), &state);
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

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["chrome", "tabs"], js_name = query)]
    fn tabs_query(query: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = ["chrome", "debugger"], js_name = attach)]
    fn debugger_attach(target: &JsValue, version: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = ["chrome", "debugger"], js_name = detach)]
    fn debugger_detach(target: &JsValue) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = ["chrome", "debugger"], js_name = getTargets)]
    fn debugger_get_targets() -> js_sys::Promise;

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

    #[wasm_bindgen(js_namespace = ["chrome", "storage", "local"], js_name = set)]
    fn storage_local_set(items: &JsValue) -> js_sys::Promise;
}
