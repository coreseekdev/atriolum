use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "atriolum-server", about = "Sentry-compatible error tracking server")]
pub struct Cli {
    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to listen on
    #[arg(long, default_value_t = 8000)]
    pub port: u16,

    /// Data directory for event storage
    #[arg(long, default_value = "./data")]
    pub data_dir: PathBuf,
}
