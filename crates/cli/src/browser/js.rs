pub fn escape_selector(selector: &str) -> String {
    selector.replace('\\', "\\\\").replace('\'', "\\'")
}

pub fn console_capture_injection_js() -> &'static str {
    r#"() => {
            window.__consoleMessages = [];
            const originalConsole = { ...console };
            ['log', 'warn', 'error', 'info', 'debug'].forEach(method => {
                console[method] = (...args) => {
                    window.__consoleMessages.push({
                        type: method,
                        text: args.map(a => String(a)).join(' ')
                    });
                    originalConsole[method](...args);
                };
            });
            window.onerror = (msg, src, line, col, error) => {
                window.__consoleMessages.push({
                    type: 'pageerror',
                    text: String(msg),
                    stack: error?.stack || null
                });
            };
        }"#
}

pub fn get_element_coords_js(selector: &str) -> String {
    let escaped = escape_selector(selector);
    format!(
        r#"(() => {{
                const el = document.querySelector('{escaped}');
                if (!el) return 'null';
                const rect = el.getBoundingClientRect();
                return JSON.stringify({{
                    x: Math.round(rect.x + rect.width / 2),
                    y: Math.round(rect.y + rect.height / 2),
                    width: Math.round(rect.width),
                    height: Math.round(rect.height),
                    text: el.textContent?.trim().substring(0, 100) || null,
                    href: el.getAttribute('href')
                }});
            }})()"#
    )
}

pub fn get_all_element_coords_js(selector: &str) -> String {
    let escaped = escape_selector(selector);
    format!(
        r#"(() => {{
                const elements = document.querySelectorAll('{escaped}');
                return JSON.stringify(Array.from(elements).map((el, index) => {{
                    const rect = el.getBoundingClientRect();
                    return {{
                        index,
                        x: Math.round(rect.x + rect.width / 2),
                        y: Math.round(rect.y + rect.height / 2),
                        width: Math.round(rect.width),
                        height: Math.round(rect.height),
                        text: el.textContent?.trim().substring(0, 80) || null,
                        href: el.getAttribute('href')
                    }};
                }}));
            }})()"#
    )
}
