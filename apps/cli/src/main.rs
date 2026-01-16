use clap::Parser;
use dnx_core::logic::{DnxContext, run_state_machine};
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(author, version, about = "Intel DnX Protocol Tool (Pure Rust)", long_about = None)]
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

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();

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

    info!("Starting DnX-rs Tool (nusb backend)...");

    let ctx = DnxContext {
        fw_dnx_path: args.fw_dnx,
        fw_image_path: args.fw_image,
        os_dnx_path: args.os_dnx,
        os_image_path: args.os_image,
        misc_dnx_path: args.misc_dnx,
    };

    if let Err(e) = run_state_machine(&ctx) {
        error!("Error: {}", e);
        std::process::exit(1);
    }
}
