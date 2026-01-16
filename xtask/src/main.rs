use anyhow::Result;
use clap::{Parser, Subcommand};
use std::process::Command;

#[derive(Parser)]
#[command(name = "xtask")]
#[command(about = "Tasks for the project", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build the project
    Build,
    /// Run the CLI
    Run,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Build => {
            println!("Building project...");
            let status = Command::new("cargo").arg("build").status()?;
            if !status.success() {
                anyhow::bail!("Build failed");
            }
        }
        Commands::Run => {
            println!("Running CLI...");
            let status = Command::new("cargo")
                .arg("run")
                .arg("-p")
                .arg("dnx-cli")
                .status()?;
            if !status.success() {
                anyhow::bail!("Run failed");
            }
        }
    }

    Ok(())
}
