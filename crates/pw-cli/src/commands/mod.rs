mod auth;
mod click;
mod console;
mod coords;
mod eval;
mod html;
mod navigate;
mod screenshot;
mod text;
mod wait;

use std::path::Path;

use crate::cli::{AuthAction, Commands};
use crate::error::Result;

pub async fn dispatch(command: Commands, auth_file: Option<&Path>) -> Result<()> {
    match command {
        Commands::Navigate { url } => navigate::execute(&url, auth_file).await,
        Commands::Console { url, timeout_ms } => console::execute(&url, timeout_ms, auth_file).await,
        Commands::Eval { url, expression } => eval::execute(&url, &expression, auth_file).await,
        Commands::Html { url, selector } => html::execute(&url, &selector, auth_file).await,
        Commands::Coords { url, selector } => coords::execute_single(&url, &selector, auth_file).await,
        Commands::CoordsAll { url, selector } => coords::execute_all(&url, &selector, auth_file).await,
        Commands::Screenshot { url, output } => screenshot::execute(&url, &output, auth_file).await,
        Commands::Click { url, selector } => click::execute(&url, &selector, auth_file).await,
        Commands::Text { url, selector } => text::execute(&url, &selector, auth_file).await,
        Commands::Wait { url, condition } => wait::execute(&url, &condition, auth_file).await,
        Commands::Auth { action } => match action {
            AuthAction::Login { url, output, timeout } => {
                auth::login(&url, &output, timeout).await
            }
            AuthAction::Cookies { url, format } => {
                auth::cookies(&url, &format, auth_file).await
            }
            AuthAction::Show { file } => auth::show(&file).await,
        },
    }
}
