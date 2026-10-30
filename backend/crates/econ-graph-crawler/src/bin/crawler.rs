// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! `crawler`: enqueue crawl jobs, inspect the queue, list sources, run one fetch for debugging,
//! check catalog selections against the live sources, or apply the data migrations. See [`econ_graph_crawler::cli`].

use clap::Parser;
use econ_graph_crawler::cli::{Cli, ExitStatus};

#[tokio::main]
async fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    match Cli::parse().run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => match e.downcast_ref::<ExitStatus>() {
            // The command printed its output; it only sets the exit status.
            Some(status) => {
                tracing::warn!("{status}");
                std::process::ExitCode::from(status.code)
            }
            None => {
                tracing::error!("{e:#}");
                std::process::ExitCode::FAILURE
            }
        },
    }
}
