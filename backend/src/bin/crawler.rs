//! Crawler binary for downloading economic data series

use clap::Parser;
use econ_graph_backend::services::crawler::CrawlerCli;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = CrawlerCli::parse();
    cli.run().await?;
    Ok(())
}
