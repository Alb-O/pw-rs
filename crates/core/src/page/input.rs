//! Keyboard and mouse input methods for [`Page`].

use pw_runtime::Result;

use super::Page;

fn merge_options(params: &mut serde_json::Value, opts_json: serde_json::Value) {
	if let (Some(obj), Some(opts_obj)) = (params.as_object_mut(), opts_json.as_object()) {
		obj.extend(opts_obj.clone());
	}
}

impl Page {
	pub(crate) async fn keyboard_down(&self, key: &str) -> Result<()> {
		self.channel()
			.send_no_result("keyboardDown", serde_json::json!({ "key": key }))
			.await
	}

	pub(crate) async fn keyboard_up(&self, key: &str) -> Result<()> {
		self.channel()
			.send_no_result("keyboardUp", serde_json::json!({ "key": key }))
			.await
	}

	pub(crate) async fn keyboard_press(
		&self,
		key: &str,
		options: Option<crate::KeyboardOptions>,
	) -> Result<()> {
		let mut params = serde_json::json!({ "key": key });
		if let Some(opts) = options {
			merge_options(&mut params, opts.to_json());
		}
		self.channel().send_no_result("keyboardPress", params).await
	}

	pub(crate) async fn keyboard_type(
		&self,
		text: &str,
		options: Option<crate::KeyboardOptions>,
	) -> Result<()> {
		let mut params = serde_json::json!({ "text": text });
		if let Some(opts) = options {
			merge_options(&mut params, opts.to_json());
		}
		self.channel().send_no_result("keyboardType", params).await
	}

	pub(crate) async fn keyboard_insert_text(&self, text: &str) -> Result<()> {
		self.channel()
			.send_no_result("keyboardInsertText", serde_json::json!({ "text": text }))
			.await
	}

	pub(crate) async fn mouse_move(
		&self,
		x: i32,
		y: i32,
		options: Option<crate::MouseOptions>,
	) -> Result<()> {
		let mut params = serde_json::json!({ "x": x, "y": y });
		if let Some(opts) = options {
			merge_options(&mut params, opts.to_json());
		}
		self.channel().send_no_result("mouseMove", params).await
	}

	pub(crate) async fn mouse_click(
		&self,
		x: i32,
		y: i32,
		options: Option<crate::MouseOptions>,
	) -> Result<()> {
		let mut params = serde_json::json!({ "x": x, "y": y });
		if let Some(opts) = options {
			merge_options(&mut params, opts.to_json());
		}
		self.channel().send_no_result("mouseClick", params).await
	}

	pub(crate) async fn mouse_dblclick(
		&self,
		x: i32,
		y: i32,
		options: Option<crate::MouseOptions>,
	) -> Result<()> {
		let mut params = serde_json::json!({ "x": x, "y": y, "clickCount": 2 });
		if let Some(opts) = options {
			merge_options(&mut params, opts.to_json());
		}
		self.channel().send_no_result("mouseClick", params).await
	}

	pub(crate) async fn mouse_down(&self, options: Option<crate::MouseOptions>) -> Result<()> {
		let mut params = serde_json::json!({});
		if let Some(opts) = options {
			merge_options(&mut params, opts.to_json());
		}
		self.channel().send_no_result("mouseDown", params).await
	}

	pub(crate) async fn mouse_up(&self, options: Option<crate::MouseOptions>) -> Result<()> {
		let mut params = serde_json::json!({});
		if let Some(opts) = options {
			merge_options(&mut params, opts.to_json());
		}
		self.channel().send_no_result("mouseUp", params).await
	}

	pub(crate) async fn mouse_wheel(&self, delta_x: i32, delta_y: i32) -> Result<()> {
		self.channel()
			.send_no_result(
				"mouseWheel",
				serde_json::json!({ "deltaX": delta_x, "deltaY": delta_y }),
			)
			.await
	}
}
