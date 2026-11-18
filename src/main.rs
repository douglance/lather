#![forbid(unsafe_code)]
#![forbid(unused_must_use)]
#![forbid(unreachable_code)]

mod cli;
mod hygiene;
mod integrity;
mod measure;
mod model;
mod process;
mod propose;
mod repo;
mod rust_policy;
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
