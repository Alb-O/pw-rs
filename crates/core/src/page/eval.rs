//! JavaScript evaluation methods for [`Page`].

use pw_runtime::Result;

use super::Page;

impl Page {
	/// Evaluates JavaScript in the page context, discarding the result.
	///
	/// See <https://playwright.dev/docs/api/class-page#page-evaluate>
	pub async fn evaluate(&self, expression: &str) -> Result<()> {
		self.main_frame()
			.await?
			.frame_evaluate_expression(expression)
			.await
	}

	/// Evaluates JavaScript and returns the result as a string.
	///
	/// See <https://playwright.dev/docs/api/class-page#page-evaluate>
	pub async fn evaluate_value(&self, expression: &str) -> Result<String> {
		self.main_frame()
			.await?
			.frame_evaluate_expression_value(expression)
			.await
	}

	/// Evaluates JavaScript and returns [`serde_json::Value`].
	///
	/// See <https://playwright.dev/docs/api/class-page#page-evaluate>
	///
	/// # Errors
	///
	/// Returns [`Error::ProtocolError`](pw_runtime::Error::ProtocolError) if
	/// the expression throws or returns non-serializable values.
	pub async fn evaluate_json(&self, expression: &str) -> Result<serde_json::Value> {
		self.main_frame()
			.await?
			.frame_evaluate_expression_json(expression)
			.await
	}

	/// Evaluates JavaScript and deserializes the result to type `T`.
	///
	/// See <https://playwright.dev/docs/api/class-page#page-evaluate>
	///
	/// # Errors
	///
	/// Returns error if the expression throws or the result cannot be
	/// deserialized to `T`.
	pub async fn evaluate_typed<T: serde::de::DeserializeOwned>(
		&self,
		expression: &str,
	) -> Result<T> {
		self.main_frame()
			.await?
			.frame_evaluate_expression_typed(expression)
			.await
	}
}
