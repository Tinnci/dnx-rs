use clap::Parser;
use dnx_core::events::{DnxEvent, DnxObserver, DnxPhase, LogLevel};
use dnx_core::session::{DnxSession, SessionConfig};
use std::sync::Arc;
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(
    name = "dnx",
    author,
    version,
    about = "Intel DnX Protocol Tool (Pure Rust)",
    long_about = "A modern Rust implementation of Intel xFSTK for Medfield/Merrifield platform recovery."
)]
struct Args {
    /// Path to FW DnX binary (dnx_fwr.bin)
    #[arg(long)]
    fw_dnx: Option<String>,

    /// Path to FW image (ifwi.bin)
    #[arg(long)]
    fw_image: Option<String>,

    /// Path to OS DnX binary
    #[arg(long)]
    os_dnx: Option<String>,

    /// Path to OS image (droidboot.img)
    #[arg(long)]
    os_image: Option<String>,

    /// Path to Misc DnX binary
    #[arg(long)]
    misc_dnx: Option<String>,

    /// General Purpose flags (hex)
    #[arg(long, default_value = "0")]
    gp_flags: u32,

    /// Enable IFWI wipe mode
    #[arg(long)]
    ifwi_wipe: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

/// CLI observer that prints progress to stderr.
struct CliObserver {
    verbose: bool,
}

impl DnxObserver for CliObserver {
    fn on_event(&self, event: &DnxEvent) {
        match event {
            DnxEvent::DeviceConnected { vid, pid } => {
                eprintln!("✓ Device connected: {:04X}:{:04X}", vid, pid);
            }
            DnxEvent::DeviceDisconnected => {
                eprintln!("✗ Device disconnected");
            }
            DnxEvent::PhaseChanged { from, to } => {
                if self.verbose {
                    eprintln!("→ Phase: {} → {}", from, to);
                }
            }
            DnxEvent::Progress {
                phase,
                operation,
                current,
                total,
            } => {
                let pct = if *total > 0 {
                    (*current * 100) / *total
                } else {
                    0
                };
                eprint!("\r[{:>3}%] {}: {}", pct, phase, operation);
                if *current == *total {
                    eprintln!(); // Newline when complete
                }
            }
            DnxEvent::Log { level, message } => match level {
                LogLevel::Error => eprintln!("ERROR: {}", message),
                LogLevel::Warn => eprintln!("WARN: {}", message),
                LogLevel::Info if self.verbose => eprintln!("INFO: {}", message),
                LogLevel::Debug if self.verbose => eprintln!("DEBUG: {}", message),
                LogLevel::Trace if self.verbose => eprintln!("TRACE: {}", message),
                _ => {}
            },
            DnxEvent::AckReceived { ack } => {
                if self.verbose {
                    eprintln!("← ACK: {}", ack);
                }
            }
            DnxEvent::Error { code, message } => {
                eprintln!("✗ Error [{}]: {}", code, message);
            }
            DnxEvent::Complete => {
                eprintln!("✓ Operation complete!");
            }
        }
    }
}

fn main() {
    let args = Args::parse();

    // Initialize tracing subscriber
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(if args.verbose {
                    tracing::Level::DEBUG.into()
                } else {
                    tracing::Level::INFO.into()
                })
                .from_env_lossy(),
        )
        .with_writer(std::io::stderr)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("DnX-rs Tool starting...");

    let config = SessionConfig {
        fw_dnx_path: args.fw_dnx,
        fw_image_path: args.fw_image,
        os_dnx_path: args.os_dnx,
        os_image_path: args.os_image,
        misc_dnx_path: args.misc_dnx,
        gp_flags: args.gp_flags,
        ifwi_wipe_enable: args.ifwi_wipe,
        retry_timeout_secs: 300, // 5 minutes
    };

    let observer = Arc::new(CliObserver {
        verbose: args.verbose,
    });
    let mut session = DnxSession::with_observer(config, observer);

    if let Err(e) = session.run() {
        error!("Session failed: {}", e);
        eprintln!("✗ FAILED: {}", e);
        std::process::exit(1);
    }
}
