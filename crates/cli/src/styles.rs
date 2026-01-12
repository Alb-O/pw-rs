//! CLI help output styling to match cargo's visual style.

use clap::builder::{Styles, styling::AnsiColor};

/// Returns clap Styles configured to match cargo's help output colors.
///
/// Styling breakdown:
/// - Headers (like "Usage:", "Arguments:", "Options:"): Green + Bold
/// - Usage text: Green + Bold
/// - Literals (actual command text): Cyan
/// - Placeholders (like <FILE>, <NUM>): Cyan
/// - Valid values: Cyan
pub fn cli_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Green.on_default().bold())
        .usage(AnsiColor::Green.on_default().bold())
        .literal(AnsiColor::Cyan.on_default())
        .placeholder(AnsiColor::Cyan.on_default())
        .valid(AnsiColor::Cyan.on_default())
}
