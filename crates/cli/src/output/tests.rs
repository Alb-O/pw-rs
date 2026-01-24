use super::*;

#[test]
fn result_builder_success() {
	let result: CommandResult<NavigateData> = ResultBuilder::new("navigate")
		.inputs(CommandInputs {
			url: Some("https://example.com".into()),
			..Default::default()
		})
		.data(NavigateData {
			url: "https://example.com".into(),
			actual_url: None,
			title: "Example".into(),
			errors: vec![],
			warnings: vec![],
		})
		.build();

	assert!(result.ok);
	assert_eq!(result.command, "navigate");
	assert!(result.data.is_some());
	assert!(result.error.is_none());
}

#[test]
fn result_builder_error() {
	let result: CommandResult<NavigateData> = ResultBuilder::new("navigate")
		.inputs(CommandInputs {
			url: Some("https://blocked.com".into()),
			..Default::default()
		})
		.error(ErrorCode::NavigationFailed, "Connection refused")
		.build();

	assert!(!result.ok);
	assert!(result.data.is_none());
	assert!(result.error.is_some());
	assert_eq!(
		result.error.as_ref().unwrap().code,
		ErrorCode::NavigationFailed
	);
}

#[test]
fn error_code_display() {
	assert_eq!(ErrorCode::NavigationFailed.to_string(), "NAVIGATION_FAILED");
	assert_eq!(
		ErrorCode::SelectorNotFound.to_string(),
		"SELECTOR_NOT_FOUND"
	);
}

#[test]
fn output_format_parse() {
	assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
	assert_eq!("text".parse::<OutputFormat>().unwrap(), OutputFormat::Text);
	assert!("invalid".parse::<OutputFormat>().is_err());
}

#[test]
fn serialize_command_result() {
	let result: CommandResult<ClickData> = ResultBuilder::new("click")
		.data(ClickData {
			before_url: "https://example.com".into(),
			after_url: "https://example.com/page".into(),
			navigated: true,
			selector: "a.link".into(),
			downloads: Vec::new(),
		})
		.build();

	let json = serde_json::to_string(&result).unwrap();
	assert!(json.contains("\"ok\":true"));
	assert!(json.contains("\"navigated\":true"));
}

#[test]
fn artifacts_included() {
	let result: CommandResult<ScreenshotData> = ResultBuilder::new("screenshot")
		.data(ScreenshotData {
			path: "/tmp/screenshot.png".into(),
			full_page: false,
			width: Some(1920),
			height: Some(1080),
		})
		.artifact(Artifact {
			artifact_type: ArtifactType::Screenshot,
			path: "/tmp/screenshot.png".into(),
			size_bytes: Some(12345),
		})
		.build();

	assert_eq!(result.artifacts.len(), 1);
	assert_eq!(result.artifacts[0].artifact_type, ArtifactType::Screenshot);
}

#[test]
fn diagnostics_included() {
	let result: CommandResult<NavigateData> = ResultBuilder::new("navigate")
		.data(NavigateData {
			url: "https://example.com".into(),
			actual_url: None,
			title: "Example".into(),
			errors: vec![],
			warnings: vec![],
		})
		.diagnostic(DiagnosticLevel::Warning, "Page loaded slowly")
		.diagnostic_with_source(DiagnosticLevel::Error, "JS error occurred", "browser")
		.build();

	assert_eq!(result.diagnostics.len(), 2);
	assert_eq!(result.diagnostics[0].level, DiagnosticLevel::Warning);
	assert_eq!(result.diagnostics[1].source, Some("browser".into()));
}
