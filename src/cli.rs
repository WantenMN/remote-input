use std::net::IpAddr;

use clap::Parser;

/// Remote Input Daemon — type on your phone, paste into your desktop.
#[derive(Parser, Debug)]
#[command(name = "remote-input", version, about)]
pub struct Args {
    /// Port to listen on
    #[arg(short, long, default_value_t = 48732)]
    pub port: u16,

    /// Delay in milliseconds between clipboard write and paste simulation (Linux only)
    #[arg(short = 'D', long, default_value_t = 20)]
    pub paste_delay: u64,

    /// Use HTTP instead of HTTPS (insecure, not recommended)
    #[arg(long, default_value_t = false)]
    pub http: bool,

    /// Maximum number of distinct client IPs allowed at once
    #[arg(short = 'm', long, default_value_t = 1)]
    pub max_connections: usize,

    /// Only allow connections from these IPs (comma-separated, e.g. -a 192.168.1.5,192.168.1.10)
    #[arg(short = 'a', long = "allow", value_name = "IP", value_delimiter = ',')]
    pub allow: Vec<IpAddr>,
}
