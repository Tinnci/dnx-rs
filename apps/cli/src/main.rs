use clap::{Parser, Subcommand};
use dnx_core::events::{DnxEvent, DnxObserver, LogLevel};
use dnx_core::session::{DnxSession, SessionConfig};
use std::path::Path;
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
    #[command(subcommand)]
    command: Option<Commands>,

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

    /// Hardware profile to use (e.g., 'eaglespeak', 'blackburn')
    #[arg(short, long)]
    profile: Option<String>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Download firmware/OS to device (default behavior)
    Download {
        /// Hardware profile to use
        #[arg(short, long)]
        profile: Option<String>,
    },

    /// Dump IFWI version information from firmware image
    #[command(name = "ifwi-version")]
    IfwiVersion {
        /// Path to IFWI/DnX image file
        #[arg(required = true)]
        file: String,

        /// Output in JSON format
        #[arg(long)]
        json: bool,

        /// Output in markdown format
        #[arg(long)]
        markdown: bool,
    },

    /// Analyze firmware file structure
    Analyze {
        /// Path to firmware file
        #[arg(required = true)]
        file: String,
    },
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

fn cmd_ifwi_version(
    file: &str,
    json: bool,
    markdown: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(file);

    if !path.exists() {
        return Err(format!("File not found: {}", file).into());
    }

    let data = std::fs::read(path)?;
    let versions = dnx_core::get_image_fw_rev(&data)?;

    if json {
        // JSON output
        println!("{{");
        println!("  \"ifwi\": \"{}\",", versions.ifwi);
        println!("  \"scu\": \"{}\",", versions.scu);
        println!("  \"hooks_oem\": \"{}\",", versions.valhooks);
        println!("  \"ia32\": \"{}\",", versions.ia32);
        println!("  \"chaabi\": \"{}\",", versions.chaabi);
        println!("  \"mia\": \"{}\"", versions.mia);
        println!("}}");
    } else if markdown {
        // Markdown table output
        println!("{}", versions.to_markdown());
    } else {
        // Default: human-readable
        versions.dump();
    }

    Ok(())
}

fn cmd_analyze(file: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(file);

    if !path.exists() {
        return Err(format!("File not found: {}", file).into());
    }

    // Use the unified FirmwareAnalysis API
    let analysis = dnx_core::FirmwareAnalysis::analyze(path)?;

    // Print results
    println!("{}", analysis.to_text());

    Ok(())
}

fn cmd_download(args: &Args, profile: Option<&String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut fw_dnx = args.fw_dnx.clone();
    let mut os_image = args.os_image.clone();

    let effective_profile = profile.or(args.profile.as_ref());

    if let Some(profile) = effective_profile {
        match profile.as_str() {
            "eaglespeak" => {
                fw_dnx = fw_dnx.or(Some("assets/firmware/eaglespeak/dnx_fwr.bin".to_string()));
                os_image = os_image.or(Some("assets/firmware/eaglespeak/dnx_osr.img".to_string()));
                info!("Using profile: eaglespeak (Atom Z3580)");
            }
            "blackburn" => {
                fw_dnx = fw_dnx.or(Some("assets/firmware/blackburn/dnx_fwr.bin".to_string()));
                os_image = os_image.or(Some("assets/firmware/blackburn/dnx_osr.img".to_string()));
                info!("Using profile: blackburn (Atom Z3530)");
            }
            _ => {
                error!("Unknown profile: {}", profile);
                return Err(format!(
                    "Unknown profile '{}'. Available: eaglespeak, blackburn",
                    profile
                )
                .into());
            }
        }
    }

    let config = SessionConfig {
        fw_dnx_path: fw_dnx,
        fw_image_path: args.fw_image.clone(),
        os_dnx_path: args.os_dnx.clone(),
        os_image_path: os_image,
        misc_dnx_path: args.misc_dnx.clone(),
        gp_flags: args.gp_flags,
        ifwi_wipe_enable: args.ifwi_wipe,
        retry_timeout_secs: 300,
    };

    let observer = Arc::new(CliObserver {
        verbose: args.verbose,
    });
    let mut session = DnxSession::with_observer(config, observer);

    session.run()?;
    Ok(())
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

    let result = match &args.command {
        Some(Commands::IfwiVersion {
            file,
            json,
            markdown,
        }) => cmd_ifwi_version(file, *json, *markdown),
        Some(Commands::Analyze { file }) => cmd_analyze(file),
        Some(Commands::Download { profile }) => cmd_download(&args, profile.as_ref()),
        None => {
            // Default behavior: run download
            cmd_download(&args, args.profile.as_ref())
        }
    };

    if let Err(e) = result {
        error!("Command failed: {}", e);
        eprintln!("✗ FAILED: {}", e);
        std::process::exit(1);
    }
}
