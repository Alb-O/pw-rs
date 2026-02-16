use std::io::ErrorKind;
use std::time::Duration;

use anyhow::{Context, Result};
use jsonrpsee::core::ClientError;
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};

use super::DAEMON_TCP_PORT;

const DAEMON_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) fn daemon_endpoint_url() -> String {
	format!("http://127.0.0.1:{DAEMON_TCP_PORT}")
}

pub(crate) fn connect_client() -> Result<HttpClient> {
	build_client(None)
}

pub(crate) fn connect_probe_client() -> Result<HttpClient> {
	build_client(Some(DAEMON_PROBE_TIMEOUT))
}

fn build_client(request_timeout: Option<Duration>) -> Result<HttpClient> {
	let mut builder = HttpClientBuilder::default();
	if let Some(timeout) = request_timeout {
		builder = builder.request_timeout(timeout);
	}
	builder.build(daemon_endpoint_url()).context("Failed to create daemon RPC client")
}

pub(crate) fn is_not_running_error(err: &ClientError) -> bool {
	if matches!(err, ClientError::RestartNeeded(_) | ClientError::RequestTimeout | ClientError::ParseError(_)) {
		return true;
	}

	if let ClientError::Transport(transport_err) = err {
		if let Some(io_err) = transport_err.downcast_ref::<std::io::Error>() {
			if matches!(
				io_err.kind(),
				ErrorKind::ConnectionRefused | ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted | ErrorKind::NotConnected | ErrorKind::TimedOut
			) {
				return true;
			}
		}
	}

	let msg = err.to_string().to_ascii_lowercase();
	msg.contains("connection refused")
		|| msg.contains("connection reset")
		|| msg.contains("error trying to connect")
		|| msg.contains("dns error")
		|| msg.contains("tcp connect error")
		|| msg.contains("request timeout")
		|| msg.contains("connection closed before message completed")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn request_timeout_is_not_running() {
		assert!(is_not_running_error(&ClientError::RequestTimeout));
	}

	#[test]
	fn transport_connection_refused_is_not_running() {
		let err = ClientError::Transport(Box::new(std::io::Error::new(ErrorKind::ConnectionRefused, "refused")));
		assert!(is_not_running_error(&err));
	}
}
