mod click;
mod console;
mod coords;
mod eval;
mod html;
mod navigate;
mod screenshot;
mod text;
mod wait;

use crate::cli::Commands;
use crate::error::Result;

pub async fn dispatch(command: Commands) -> Result<()> {
    match command {
        Commands::Navigate { url } => navigate::execute(&url).await,
        Commands::Console { url, timeout_ms } => console::execute(&url, timeout_ms).await,
        Commands::Eval { url, expression } => eval::execute(&url, &expression).await,
        Commands::Html { url, selector } => html::execute(&url, &selector).await,
        Commands::Coords { url, selector } => coords::execute_single(&url, &selector).await,
        Commands::CoordsAll { url, selector } => coords::execute_all(&url, &selector).await,
        Commands::Screenshot { url, output } => screenshot::execute(&url, &output).await,
        Commands::Click { url, selector } => click::execute(&url, &selector).await,
        Commands::Text { url, selector } => text::execute(&url, &selector).await,
        Commands::Wait { url, condition } => wait::execute(&url, &condition).await,
    }
}
