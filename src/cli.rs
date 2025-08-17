use clap::{ArgAction, Parser};

#[derive(Parser, Debug)]
#[command(author = "Tarush Mohindru <tarushmohindru1515@gmail.com>",
 version = "0.1.0", 
 about = "A fast multi-threaded downloader", long_about = None)]
pub struct Cli {
    /// URL to download
    #[arg(required = true, index = 1)]
    pub url: String,

    /// Output file path
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<String>,

    /// Maximum number of concurrent connections
    #[arg(short, long, value_name = "NUM", default_value = "16")]
    pub connections: usize,

    /// Chunk size in bytes (supports K, M, G suffixes)
    #[arg(long, value_name = "SIZE", default_value = "1M")]
    pub chunk_size: String,

    /// Custom User-Agent header
    #[arg(short, long, value_name = "STRING")]
    pub user_agent: Option<String>,

    /// Custom HTTP header (format: 'Name: Value')
    #[arg(long, value_name = "HEADER", action = ArgAction::Append)]
    pub header: Option<Vec<String>>,

    /// Disable resume support
    #[arg(long, action = ArgAction::SetTrue)]
    pub no_resume: bool,

    /// Disable adaptive chunking
    #[arg(long, action = ArgAction::SetTrue)]
    pub no_adaptive: bool,

    /// Number of retry attempts
    #[arg(short, long, value_name = "NUM", default_value = "3")]
    pub retries: usize,

    /// Connection timeout in seconds
    #[arg(short, long, value_name = "SECONDS", default_value = "30")]
    pub timeout: u64,

    /// Quiet mode - no progress bar
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub quiet: bool,

    /// Verbose output
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub verbose: bool,
}
