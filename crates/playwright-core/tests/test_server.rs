// Test Server - Local HTTP server for integration tests
//
// Provides a local HTTP server serving test HTML pages.
// This enables deterministic, offline integration testing.

use axum::{
    body::Body,
    http::{Response, StatusCode},
    routing::get,
    Router,
};
use std::net::SocketAddr;
use tokio::task::JoinHandle;

/// Test server handle
pub struct TestServer {
    addr: SocketAddr,
    handle: JoinHandle<()>,
}

impl TestServer {
    /// Start the test server on a random available port
    pub async fn start() -> Self {
        let app = Router::new()
            .route("/", get(index_page))
            .route("/button.html", get(button_page))
            .route("/form.html", get(form_page))
            .route("/input.html", get(input_page))
            .route("/dblclick.html", get(dblclick_page))
            .route("/keyboard.html", get(keyboard_page))
            .route("/locator.html", get(locator_page))
            .route("/checkbox.html", get(checkbox_page))
            .route("/hover.html", get(hover_page))
            .route("/select.html", get(select_page))
            .route("/upload.html", get(upload_page));

        // Bind to port 0 to get any available port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind test server");

        let addr = listener.local_addr().expect("Failed to get local address");

        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("Test server failed");
        });

        TestServer { addr, handle }
    }

    /// Get the base URL of the test server
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Shutdown the test server
    pub fn shutdown(self) {
        self.handle.abort();
    }
}

// Test HTML pages

async fn index_page() -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html")
        .body(Body::from(
            r#"<!DOCTYPE html>
<html>
<head><title>Test Index</title></head>
<body>
  <h1>Test Page</h1>
  <p>This is a test paragraph.</p>
  <a href="/button.html">Go to button page</a>
</body>
</html>"#,
        ))
        .unwrap()
}

async fn button_page() -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html")
        .body(Body::from(
            r#"<!DOCTYPE html>
<html>
<head><title>Button Test</title></head>
<body>
  <button id="btn" onclick="this.textContent='clicked'">Click me</button>
  <button id="btn2" onclick="this.textContent='clicked 2'">Click me 2</button>
</body>
</html>"#,
        ))
        .unwrap()
}

async fn form_page() -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html")
        .body(Body::from(
            r#"<!DOCTYPE html>
<html>
<head><title>Form Test</title></head>
<body>
  <form>
    <input type="text" id="name" name="name" />
    <textarea id="bio" name="bio"></textarea>
    <input type="submit" value="Submit" />
  </form>
</body>
</html>"#,
        ))
        .unwrap()
}

async fn input_page() -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html")
        .body(Body::from(
            r#"<!DOCTYPE html>
<html>
<head><title>Input Test</title></head>
<body>
  <input type="text" id="input" value="initial" />
  <input type="text" id="empty" value="" />
</body>
</html>"#,
        ))
        .unwrap()
}

async fn dblclick_page() -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html")
        .body(Body::from(
            r#"<!DOCTYPE html>
<html>
<head><title>Double Click Test</title></head>
<body>
  <div id="target" ondblclick="this.textContent='double clicked'">Double click me</div>
</body>
</html>"#,
        ))
        .unwrap()
}

async fn keyboard_page() -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html")
        .body(Body::from(
            r#"<!DOCTYPE html>
<html>
<head><title>Keyboard Test</title></head>
<body>
  <input type="text" id="input" onkeydown="if(event.key==='Enter') this.value='submitted'" />
</body>
</html>"#,
        ))
        .unwrap()
}

async fn locator_page() -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html")
        .body(Body::from(
            r#"<!DOCTYPE html>
<html>
<head><title>Locator Test</title></head>
<body>
  <h1>Test Page</h1>
  <p id="p1">First paragraph</p>
  <p id="p2">Second paragraph</p>
  <p id="p3">Third paragraph</p>
  <div class="container">
    <span id="nested">Nested element</span>
  </div>
  <div id="hidden" style="display: none;">Hidden element</div>
</body>
</html>"#,
        ))
        .unwrap()
}

async fn checkbox_page() -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html")
        .body(Body::from(
            r#"<!DOCTYPE html>
<html>
<head><title>Checkbox Test</title></head>
<body>
  <input type="checkbox" id="checkbox" />
  <label for="checkbox">Unchecked checkbox</label>
  <br />
  <input type="checkbox" id="checked-checkbox" checked />
  <label for="checked-checkbox">Checked checkbox</label>
  <br />
  <input type="radio" id="radio1" name="radio-group" />
  <label for="radio1">Radio 1</label>
  <br />
  <input type="radio" id="radio2" name="radio-group" />
  <label for="radio2">Radio 2</label>
</body>
</html>"#,
        ))
        .unwrap()
}

async fn hover_page() -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html")
        .body(Body::from(
            r#"<!DOCTYPE html>
<html>
<head>
  <title>Hover Test</title>
  <style>
    #hover-button {
      padding: 10px;
      background-color: #ccc;
    }
    #tooltip {
      display: none;
      margin-top: 10px;
      padding: 5px;
      background-color: yellow;
    }
    #hover-button:hover + #tooltip {
      display: block;
    }
  </style>
</head>
<body>
  <button id="hover-button">Hover over me</button>
  <div id="tooltip">This is a tooltip</div>
</body>
</html>"#,
        ))
        .unwrap()
}

async fn select_page() -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html")
        .body(Body::from(
            r#"<!DOCTYPE html>
<html>
<head><title>Select Test</title></head>
<body>
  <select id="single-select">
    <option value="">--Please choose an option--</option>
    <option value="apple">Apple</option>
    <option value="banana">Banana</option>
    <option value="cherry">Cherry</option>
  </select>
  <br /><br />
  <select id="multi-select" multiple>
    <option value="red">Red</option>
    <option value="green">Green</option>
    <option value="blue">Blue</option>
    <option value="yellow">Yellow</option>
  </select>
  <br /><br />
  <select id="select-by-index">
    <option>First</option>
    <option>Second</option>
    <option>Third</option>
  </select>
</body>
</html>"#,
        ))
        .unwrap()
}

async fn upload_page() -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html")
        .body(Body::from(
            r#"<!DOCTYPE html>
<html>
<head><title>File Upload Test</title></head>
<body>
  <input type="file" id="single-file" />
  <br /><br />
  <input type="file" id="multi-file" multiple />
  <br /><br />
  <div id="file-info"></div>
  <script>
    document.getElementById('single-file').addEventListener('change', (e) => {
      const files = Array.from(e.target.files).map(f => f.name).join(', ');
      document.getElementById('file-info').textContent = 'Single: ' + files;
    });
    document.getElementById('multi-file').addEventListener('change', (e) => {
      const files = Array.from(e.target.files).map(f => f.name).join(', ');
      document.getElementById('file-info').textContent = 'Multiple: ' + files;
    });
  </script>
</body>
</html>"#,
        ))
        .unwrap()
}
