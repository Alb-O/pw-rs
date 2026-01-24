use std::sync::Arc;
use std::sync::atomic::Ordering;

use tokio::io::duplex;

use super::*;
use crate::transport::PipeTransport;

fn create_test_connection() -> (Connection, tokio::io::DuplexStream, tokio::io::DuplexStream) {
	let (stdin_read, stdin_write) = duplex(1024);
	let (stdout_read, stdout_write) = duplex(1024);

	let (transport, message_rx) = PipeTransport::new(stdin_write, stdout_read);
	let parts = transport.into_transport_parts(message_rx);
	let connection = Connection::new(parts);

	(connection, stdin_read, stdout_write)
}

#[test]
fn test_request_id_increments() {
	let (connection, _, _) = create_test_connection();

	let id1 = connection.last_id.fetch_add(1, Ordering::SeqCst);
	let id2 = connection.last_id.fetch_add(1, Ordering::SeqCst);
	let id3 = connection.last_id.fetch_add(1, Ordering::SeqCst);

	assert_eq!(id1, 0);
	assert_eq!(id2, 1);
	assert_eq!(id3, 2);
}

#[test]
fn test_request_format() {
	let request = Request {
		id: 0,
		guid: Arc::from("page@abc123"),
		method: "goto".to_string(),
		params: serde_json::json!({"url": "https://example.com"}),
		metadata: Metadata::now(),
	};

	assert_eq!(request.id, 0);
	assert_eq!(request.guid.as_ref(), "page@abc123");
	assert_eq!(request.method, "goto");
	assert_eq!(request.params["url"], "https://example.com");
}

#[tokio::test]
async fn test_dispatch_response_success() {
	let (connection, _, _) = create_test_connection();

	let id = connection.last_id.fetch_add(1, Ordering::SeqCst);

	let (tx, rx) = tokio::sync::oneshot::channel();
	connection.callbacks.lock().await.insert(id, tx);

	let response = Message::Response(Response {
		id,
		result: Some(serde_json::json!({"status": "ok"})),
		error: None,
	});

	Arc::new(connection).dispatch(response).await.unwrap();

	let result = rx.await.unwrap().unwrap();
	assert_eq!(result["status"], "ok");
}

#[tokio::test]
async fn test_dispatch_response_error() {
	let (connection, _, _) = create_test_connection();

	let id = connection.last_id.fetch_add(1, Ordering::SeqCst);

	let (tx, rx) = tokio::sync::oneshot::channel();
	connection.callbacks.lock().await.insert(id, tx);

	let response = Message::Response(Response {
		id,
		result: None,
		error: Some(ErrorWrapper {
			error: ErrorPayload {
				message: "Navigation timeout".to_string(),
				name: Some("TimeoutError".to_string()),
				stack: None,
			},
		}),
	});

	Arc::new(connection).dispatch(response).await.unwrap();

	let result = rx.await.unwrap();
	assert!(result.is_err());
	let err = result.unwrap_err();
	assert!(err.is_timeout(), "Expected timeout error, got: {:?}", err);
}

#[test]
fn test_message_deserialization_response() {
	let json = r#"{"id": 42, "result": {"status": "ok"}}"#;
	let message: Message = serde_json::from_str(json).unwrap();

	match message {
		Message::Response(response) => {
			assert_eq!(response.id, 42);
			assert!(response.result.is_some());
			assert!(response.error.is_none());
		}
		_ => panic!("Expected Response"),
	}
}

#[test]
fn test_message_deserialization_event() {
	let json = r#"{"guid": "page@abc", "method": "console", "params": {"text": "hello"}}"#;
	let message: Message = serde_json::from_str(json).unwrap();

	match message {
		Message::Event(event) => {
			assert_eq!(event.guid.as_ref(), "page@abc");
			assert_eq!(event.method, "console");
			assert_eq!(event.params["text"], "hello");
		}
		_ => panic!("Expected Event"),
	}
}

#[test]
fn test_error_type_parsing() {
	let error = parse_protocol_error(ErrorPayload {
		message: "timeout".to_string(),
		name: Some("TimeoutError".to_string()),
		stack: Some("stack trace".to_string()),
	});
	assert!(error.is_timeout());
	match &error {
		Error::Remote {
			name,
			message,
			stack,
		} => {
			assert_eq!(name, "TimeoutError");
			assert_eq!(message, "timeout");
			assert_eq!(stack.as_deref(), Some("stack trace"));
		}
		_ => panic!("Expected Remote error"),
	}
}
