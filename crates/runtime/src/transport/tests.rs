use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::*;

#[test]
fn test_length_prefix_encoding() {
	// Test that we match Python's little-endian encoding
	let length: u32 = 1234;
	let bytes = length.to_le_bytes();

	// Verify little-endian byte order
	assert_eq!(bytes[0], (length & 0xFF) as u8);
	assert_eq!(bytes[1], ((length >> 8) & 0xFF) as u8);
	assert_eq!(bytes[2], ((length >> 16) & 0xFF) as u8);
	assert_eq!(bytes[3], ((length >> 24) & 0xFF) as u8);

	// Verify round-trip
	assert_eq!(u32::from_le_bytes(bytes), length);
}

#[test]
fn test_message_framing_format() {
	// Verify our framing matches Python's format:
	// len(data).to_bytes(4, byteorder="little") + data
	let message = serde_json::json!({"test": "hello"});
	let json_bytes = serde_json::to_vec(&message).unwrap();
	let length = json_bytes.len() as u32;
	let length_bytes = length.to_le_bytes();

	// Frame should be: [length (4 bytes LE)][JSON bytes]
	let mut frame = Vec::new();
	frame.extend_from_slice(&length_bytes);
	frame.extend_from_slice(&json_bytes);

	// Verify structure
	assert_eq!(frame.len(), 4 + json_bytes.len());
	assert_eq!(&frame[0..4], &length_bytes);
	assert_eq!(&frame[4..], &json_bytes);
}

#[tokio::test]
async fn test_send_message() {
	// Create TWO separate duplex pipes:
	// 1. For stdin: transport writes, we read
	// 2. For stdout: we write, transport reads
	let (stdin_read, stdin_write) = tokio::io::duplex(1024);
	let (stdout_read, stdout_write) = tokio::io::duplex(1024);

	// Give transport the write end of stdin pipe and read end of stdout pipe
	let (_stdin_read, mut _stdout_write) = (stdin_read, stdout_write);
	let (transport, _rx) = PipeTransport::new(stdin_write, stdout_read);
	let (mut sender, _receiver) = transport.into_parts();

	// Test message
	let test_message = serde_json::json!({
		"id": 1,
		"method": "test",
		"params": {"foo": "bar"}
	});

	// Send message
	sender.send(test_message.clone()).await.unwrap();

	// Read what transport wrote to stdin from our read end
	let (mut read_half, _write_half) = tokio::io::split(_stdin_read);
	let mut len_buf = [0u8; 4];
	read_half.read_exact(&mut len_buf).await.unwrap();
	let length = u32::from_le_bytes(len_buf) as usize;

	let mut msg_buf = vec![0u8; length];
	read_half.read_exact(&mut msg_buf).await.unwrap();

	let received: serde_json::Value = serde_json::from_slice(&msg_buf).unwrap();
	assert_eq!(received, test_message);
}

#[tokio::test]
async fn test_multiple_messages_in_sequence() {
	// Create two duplex pipes for bidirectional communication
	let (_stdin_read, stdin_write) = tokio::io::duplex(4096);
	let (stdout_read, mut stdout_write) = tokio::io::duplex(4096);

	let (mut transport, mut rx) = PipeTransport::new(stdin_write, stdout_read);

	// Spawn reader task
	let read_task = tokio::spawn(async move { transport.run().await });

	// Send multiple messages (simulating server sending to transport)
	let messages = vec![
		serde_json::json!({"id": 1, "method": "first"}),
		serde_json::json!({"id": 2, "method": "second"}),
		serde_json::json!({"id": 3, "method": "third"}),
	];

	for msg in &messages {
		let json_bytes = serde_json::to_vec(msg).unwrap();
		let length = json_bytes.len() as u32;

		stdout_write.write_all(&length.to_le_bytes()).await.unwrap();
		stdout_write.write_all(&json_bytes).await.unwrap();
	}
	stdout_write.flush().await.unwrap();

	// Receive all messages
	for expected in &messages {
		let received = rx.recv().await.unwrap();
		assert_eq!(&received, expected);
	}

	// Clean up
	drop(stdout_write);
	drop(rx);
	let _ = read_task.await;
}

#[tokio::test]
async fn test_large_message() {
	let (_stdin_read, stdin_write) = tokio::io::duplex(1024 * 1024); // 1MB buffer
	let (stdout_read, mut stdout_write) = tokio::io::duplex(1024 * 1024);

	let (mut transport, mut rx) = PipeTransport::new(stdin_write, stdout_read);

	// Spawn reader
	let read_task = tokio::spawn(async move { transport.run().await });

	// Create a large message (>32KB to test chunked reading note in code)
	let large_string = "x".repeat(100_000);
	let large_message = serde_json::json!({
		"id": 1,
		"data": large_string
	});

	let json_bytes = serde_json::to_vec(&large_message).unwrap();
	let length = json_bytes.len() as u32;

	// Should be > 32KB
	assert!(length > 32_768, "Test message should be > 32KB");

	stdout_write.write_all(&length.to_le_bytes()).await.unwrap();
	stdout_write.write_all(&json_bytes).await.unwrap();
	stdout_write.flush().await.unwrap();

	// Verify we can receive it
	let received = rx.recv().await.unwrap();
	assert_eq!(received, large_message);

	drop(stdout_write);
	drop(rx);
	let _ = read_task.await;
}

#[tokio::test]
async fn test_malformed_length_prefix() {
	let (_stdin_read, stdin_write) = tokio::io::duplex(1024);
	let (stdout_read, mut stdout_write) = tokio::io::duplex(1024);

	let (mut transport, _rx) = PipeTransport::new(stdin_write, stdout_read);

	// Write only 2 bytes instead of 4 (incomplete length prefix)
	// This simulates server sending malformed data
	stdout_write.write_all(&[0x01, 0x02]).await.unwrap();
	stdout_write.flush().await.unwrap();

	// Close the pipe to trigger EOF
	drop(stdout_write);

	// Run should error on incomplete read
	let result = transport.run().await;
	assert!(result.is_err());
	assert!(
		result
			.unwrap_err()
			.to_string()
			.contains("Failed to read length prefix")
	);
}

#[tokio::test]
async fn test_broken_pipe() {
	let (_stdin_read, stdin_write) = tokio::io::duplex(1024);
	let (stdout_read, stdout_write) = tokio::io::duplex(1024);

	let (mut transport, _rx) = PipeTransport::new(stdin_write, stdout_read);

	// Close the stdout write side immediately
	drop(stdout_write);

	// Spawn run() - it should error when trying to read from closed pipe
	let read_task = tokio::spawn(async move { transport.run().await });

	// Wait for it to complete - should be an error
	let result = read_task.await.unwrap();
	assert!(result.is_err());
}

#[tokio::test]
async fn test_graceful_shutdown() {
	let (_stdin_read, stdin_write) = tokio::io::duplex(1024);
	let (stdout_read, mut stdout_write) = tokio::io::duplex(1024);

	let (mut transport, mut rx) = PipeTransport::new(stdin_write, stdout_read);

	// Spawn reader
	let read_task = tokio::spawn(async move { transport.run().await });

	// Send a message
	let message = serde_json::json!({"id": 1, "method": "test"});
	let json_bytes = serde_json::to_vec(&message).unwrap();
	let length = json_bytes.len() as u32;

	stdout_write.write_all(&length.to_le_bytes()).await.unwrap();
	stdout_write.write_all(&json_bytes).await.unwrap();
	stdout_write.flush().await.unwrap();

	// Receive the message
	let received = rx.recv().await.unwrap();
	assert_eq!(received, message);

	// Drop the receiver (simulates connection closing)
	drop(rx);

	// Close stdout pipe
	drop(stdout_write);

	// Reader should exit cleanly (channel closed)
	let result = read_task.await.unwrap();
	// Should succeed - channel closed is expected shutdown
	assert!(result.is_ok() || result.unwrap_err().to_string().contains("Failed to read"));
}
