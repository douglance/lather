mod cli;
mod integrity;
mod measure;
mod model;
mod process;
mod propose;
mod repo;
mod service;
mod signals;
mod store;
mod strictness;
mod transport;
mod typescript;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = cli::app();
    cli.try_tool_catalog()?;
    cli.serve().await
}
