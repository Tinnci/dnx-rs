//! xtask - Development automation tasks for dnx-rs
//!
//! This crate provides development, testing, and release automation for the dnx-rs project.
//! Run with: `cargo xtask <command>`

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Project root directory
fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Assets directory
fn assets_dir() -> PathBuf {
    project_root().join("assets")
}

/// Firmware directory
fn firmware_dir() -> PathBuf {
    assets_dir().join("firmware")
}

#[derive(Parser)]
#[command(name = "xtask")]
#[command(about = "Development tasks for dnx-rs", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build the project
    Build {
        /// Build in release mode
        #[arg(short, long)]
        release: bool,

        /// Target to build (all, cli, tui, core)
        #[arg(short, long, default_value = "all")]
        target: BuildTarget,
    },

    /// Run tests
    Test {
        /// Run only unit tests
        #[arg(long)]
        unit: bool,

        /// Run only integration tests  
        #[arg(long)]
        integration: bool,
    },

    /// Run the CLI
    Run {
        /// Profile to use (eaglespeak, blackburn)
        #[arg(short, long)]
        profile: Option<String>,

        /// Additional arguments to pass
        #[arg(last = true)]
        args: Vec<String>,
    },

    /// Analyze firmware files
    Analyze {
        /// Firmware file or directory to analyze
        path: Option<PathBuf>,

        /// Output format (text, json, markdown)
        #[arg(short, long, default_value = "text")]
        format: OutputFormat,
    },

    /// Extract IFWI version information
    #[command(name = "ifwi-version")]
    IfwiVersion {
        /// Path to IFWI/firmware file
        file: PathBuf,

        /// Output format
        #[arg(short, long, default_value = "text")]
        format: OutputFormat,
    },

    /// Generate documentation
    Doc {
        /// Open in browser after generation
        #[arg(long)]
        open: bool,
    },

    /// Check code quality (fmt, clippy, etc.)
    Check {
        /// Auto-fix issues where possible
        #[arg(long)]
        fix: bool,
    },

    /// Release tasks
    Release {
        /// Version bump type
        #[arg(short, long)]
        bump: Option<VersionBump>,

        /// Create GitHub release
        #[arg(long)]
        github: bool,
    },

    /// Clean build artifacts
    Clean {
        /// Also clean downloaded firmware
        #[arg(long)]
        all: bool,
    },

    /// Setup development environment
    Setup,

    /// Generate firmware analysis report
    Report {
        /// Output path for the report
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Include all available firmware
        #[arg(long)]
        all: bool,
    },

    /// Firmware utilities
    Firmware {
        #[command(subcommand)]
        cmd: FirmwareCommands,
    },

    /// Generate a new integration test template
    #[command(name = "generate-test")]
    GenerateTest {
        /// Name of the test case
        #[arg(short, long)]
        name: String,
    },
}

#[derive(Subcommand)]
enum FirmwareCommands {
    /// List available firmware profiles
    List,

    /// Download firmware from source
    Download {
        /// Profile name
        profile: String,

        /// Source URL
        #[arg(long)]
        url: Option<String>,
    },

    /// Validate firmware integrity
    Validate {
        /// Profile or file path
        target: String,
    },

    /// Extract components from firmware
    Extract {
        /// Source firmware file
        source: PathBuf,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Component to extract (token, chaabi, ifwi, all)
        #[arg(short, long, default_value = "all")]
        component: String,
    },

    /// Compare two firmware files
    Compare {
        /// First firmware file
        file1: PathBuf,

        /// Second firmware file
        file2: PathBuf,

        /// Show detailed diff
        #[arg(long)]
        detailed: bool,
    },
}

#[derive(Clone, Copy, ValueEnum, Default)]
enum BuildTarget {
    #[default]
    All,
    Cli,
    Tui,
    Core,
}

#[derive(Clone, Copy, ValueEnum, Default)]
enum OutputFormat {
    #[default]
    Text,
    Json,
    Markdown,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum VersionBump {
    Patch,
    Minor,
    Major,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let _root = project_root();

    match cli.command {
        Commands::Build { release, target } => cmd_build(release, target)?,
        Commands::Test { unit, integration } => cmd_test(unit, integration)?,
        Commands::Run { profile, args } => cmd_run(profile, args)?,
        Commands::Analyze { path, format } => cmd_analyze(path, format)?,
        Commands::IfwiVersion { file, format } => cmd_ifwi_version(&file, format)?,
        Commands::Doc { open } => cmd_doc(open)?,
        Commands::Check { fix } => cmd_check(fix)?,
        Commands::Release { bump, github } => cmd_release(bump, github)?,
        Commands::Clean { all } => cmd_clean(all)?,
        Commands::Setup => cmd_setup()?,
        Commands::Report { output, all } => cmd_report(output, all)?,
        Commands::Firmware { cmd } => match cmd {
            FirmwareCommands::List => cmd_firmware_list()?,
            FirmwareCommands::Download { profile, url } => cmd_firmware_download(&profile, url)?,
            FirmwareCommands::Validate { target } => cmd_firmware_validate(&target)?,
            FirmwareCommands::Extract {
                source,
                output,
                component,
            } => cmd_firmware_extract(&source, output, &component)?,
            FirmwareCommands::Compare {
                file1,
                file2,
                detailed,
            } => cmd_firmware_compare(&file1, &file2, detailed)?,
        },
        Commands::GenerateTest { name } => cmd_generate_test(&name)?,
    }

    Ok(())
}

// ============================================================================
// Command Implementations
// ============================================================================

fn cmd_build(release: bool, target: BuildTarget) -> Result<()> {
    let root = project_root();
    println!("📦 Building project...");

    let mut cmd = Command::new("cargo");
    cmd.current_dir(&root);
    cmd.arg("build");

    if release {
        cmd.arg("--release");
    }

    match target {
        BuildTarget::All => {}
        BuildTarget::Cli => {
            cmd.args(["-p", "dnx-cli"]);
        }
        BuildTarget::Tui => {
            cmd.args(["-p", "dnx-tui"]);
        }
        BuildTarget::Core => {
            cmd.args(["-p", "dnx-core"]);
        }
    }

    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("Build failed");
    }

    println!("✅ Build complete");
    Ok(())
}

fn cmd_test(unit: bool, integration: bool) -> Result<()> {
    let root = project_root();
    println!("🧪 Running tests...");

    let mut cmd = Command::new("cargo");
    cmd.current_dir(&root);
    cmd.arg("test");

    if unit && !integration {
        cmd.arg("--lib");
    } else if integration && !unit {
        cmd.arg("--test");
        cmd.arg("*");
    }

    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("Tests failed");
    }

    println!("✅ All tests passed");
    Ok(())
}

fn cmd_run(profile: Option<String>, args: Vec<String>) -> Result<()> {
    let root = project_root();
    println!("🚀 Running dnx-cli...");

    let mut cmd = Command::new("cargo");
    cmd.current_dir(&root);
    cmd.args(["run", "-p", "dnx-cli", "--"]);

    if let Some(p) = profile {
        cmd.args(["-p", &p]);
    }

    for arg in args {
        cmd.arg(arg);
    }

    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("Run failed");
    }

    Ok(())
}

fn cmd_analyze(path: Option<PathBuf>, format: OutputFormat) -> Result<()> {
    let target = path.unwrap_or_else(firmware_dir);

    if target.is_dir() {
        println!("📊 Analyzing firmware directory: {}", target.display());
        analyze_directory(&target, format)?;
    } else {
        println!("📊 Analyzing firmware file: {}", target.display());
        analyze_file(&target, format)?;
    }

    Ok(())
}

fn analyze_directory(dir: &Path, format: OutputFormat) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            println!("\n📁 {}/", path.file_name().unwrap().to_string_lossy());
            analyze_directory(&path, format)?;
        } else if path.extension().map_or(false, |e| e == "bin" || e == "img") {
            analyze_file(&path, format)?;
        }
    }
    Ok(())
}

fn analyze_file(path: &Path, _format: OutputFormat) -> Result<()> {
    let root = project_root();
    let mut cmd = Command::new("cargo");
    cmd.current_dir(&root);
    cmd.args(["run", "-p", "dnx-cli", "-q", "--", "analyze"]);
    cmd.arg(path);

    let status = cmd.status()?;
    if !status.success() {
        println!("⚠️  Failed to analyze: {}", path.display());
    }
    Ok(())
}

fn cmd_ifwi_version(file: &Path, format: OutputFormat) -> Result<()> {
    let root = project_root();
    let mut cmd = Command::new("cargo");
    cmd.current_dir(&root);
    cmd.args(["run", "-p", "dnx-cli", "-q", "--", "ifwi-version"]);
    cmd.arg(file);

    match format {
        OutputFormat::Json => cmd.arg("--json"),
        OutputFormat::Markdown => cmd.arg("--markdown"),
        OutputFormat::Text => &mut cmd,
    };

    cmd.status()?;
    Ok(())
}

fn cmd_doc(open: bool) -> Result<()> {
    let root = project_root();
    println!("📚 Generating documentation...");

    let mut cmd = Command::new("cargo");
    cmd.current_dir(&root);
    cmd.args(["doc", "--no-deps"]);

    if open {
        cmd.arg("--open");
    }

    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("Documentation generation failed");
    }

    println!("✅ Documentation generated");
    Ok(())
}

fn cmd_check(fix: bool) -> Result<()> {
    let root = project_root();
    println!("🔍 Checking code quality...");

    // Format check
    println!("  → Checking formatting...");
    let mut fmt_cmd = Command::new("cargo");
    fmt_cmd.current_dir(&root);
    fmt_cmd.arg("fmt");
    if fix {
        fmt_cmd.status()?;
    } else {
        fmt_cmd.arg("--check");
        let status = fmt_cmd.status()?;
        if !status.success() {
            anyhow::bail!("Formatting issues found. Run with --fix to auto-fix.");
        }
    }

    // Clippy
    println!("  → Running clippy...");
    let mut clippy_cmd = Command::new("cargo");
    clippy_cmd.current_dir(&root);
    clippy_cmd.args(["clippy", "--all-targets"]);
    if fix {
        clippy_cmd.args(["--fix", "--allow-dirty"]);
    } else {
        clippy_cmd.args(["--", "-D", "warnings"]);
    }

    let status = clippy_cmd.status()?;
    if !status.success() {
        anyhow::bail!("Clippy found issues");
    }

    println!("✅ Code quality check passed");
    Ok(())
}

fn cmd_release(bump: Option<VersionBump>, github: bool) -> Result<()> {
    println!("🚀 Preparing release...");

    if let Some(bump) = bump {
        println!("  → Version bump: {:?}", bump);
        // TODO: Implement version bumping
    }

    // Build release
    cmd_build(true, BuildTarget::All)?;

    // Run tests
    cmd_test(false, false)?;

    if github {
        println!("  → Creating GitHub release...");
        // TODO: Implement GitHub release
    }

    println!("✅ Release preparation complete");
    Ok(())
}

fn cmd_clean(all: bool) -> Result<()> {
    let root = project_root();
    println!("🧹 Cleaning build artifacts...");

    let status = Command::new("cargo")
        .current_dir(&root)
        .arg("clean")
        .status()?;

    if !status.success() {
        anyhow::bail!("Clean failed");
    }

    if all {
        println!("  → Cleaning additional artifacts...");
        let _ = std::fs::remove_file(root.join("dnx-tui.log"));
    }

    println!("✅ Clean complete");
    Ok(())
}

fn cmd_setup() -> Result<()> {
    println!("🔧 Setting up development environment...");

    // Check Rust version
    println!("  → Checking Rust version...");
    let output = Command::new("rustc").arg("--version").output()?;
    println!("    {}", String::from_utf8_lossy(&output.stdout).trim());

    // Install cargo-watch if not present
    println!("  → Checking cargo-watch...");
    if Command::new("cargo")
        .args(["watch", "--version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_err()
    {
        println!("    Installing cargo-watch...");
        Command::new("cargo")
            .args(["install", "cargo-watch"])
            .status()?;
    }

    println!("✅ Setup complete");
    Ok(())
}

fn cmd_report(output: Option<PathBuf>, _all: bool) -> Result<()> {
    let _root = project_root();
    let output_path = output.unwrap_or_else(|| assets_dir().join("firmware").join("README.md"));

    println!("📝 Generating firmware analysis report...");
    println!("  → Output: {}", output_path.display());

    // The report is already generated, just inform user
    if output_path.exists() {
        println!("✅ Report exists at: {}", output_path.display());
    } else {
        println!("⚠️  Report not found. Run firmware analysis first.");
    }

    Ok(())
}

fn cmd_generate_test(name: &str) -> Result<()> {
    let root = project_root();
    let tests_dir = root.join("tests");
    std::fs::create_dir_all(&tests_dir)?;
    let test_file = tests_dir.join(format!("{}.rs", name));

    if test_file.exists() {
        anyhow::bail!("Test file already exists: {}", test_file.display());
    }

    println!("🧪 Generating test template: {}", test_file.display());

    let content = format!(
        r#"use dnx_core::{{DnxSession, SessionConfig}};
use dnx_core::transport::MockTransport;
use std::sync::Arc;

#[test]
fn test_{}() {{
    // 1. Setup Mock Transport with expected sequence
    let mock = MockTransport::new();
    
    // Example: Mock setup
    // mock.expect_write(&[...]); 

    // 2. Configure Session
    let config = SessionConfig::default();
    
    // 3. Run Test
    println!("Test template generated for {{}}", "{}");
}}
"#,
        name, name
    );

    std::fs::write(&test_file, content)?;
    println!("✅ Test template created");
    Ok(())
}

// ============================================================================
// Firmware Subcommands
// ============================================================================

fn cmd_firmware_list() -> Result<()> {
    let fw_dir = firmware_dir();
    println!("📦 Available firmware profiles:\n");

    for entry in std::fs::read_dir(&fw_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let name = path.file_name().unwrap().to_string_lossy();
            let dnx_fwr = path.join("dnx_fwr.bin");
            let dnx_osr = path.join("dnx_osr.img");

            println!("  {} {}", if dnx_fwr.exists() { "✅" } else { "❌" }, name);

            if dnx_fwr.exists() {
                let size = std::fs::metadata(&dnx_fwr)?.len();
                println!("     └─ dnx_fwr.bin ({} bytes)", size);
            }
            if dnx_osr.exists() {
                let size = std::fs::metadata(&dnx_osr)?.len();
                println!(
                    "     └─ dnx_osr.img ({:.2} MB)",
                    size as f64 / 1024.0 / 1024.0
                );
            }
        }
    }

    Ok(())
}

fn cmd_firmware_download(profile: &str, url: Option<String>) -> Result<()> {
    println!("📥 Downloading firmware for profile: {}", profile);

    if let Some(url) = url {
        println!("  Source: {}", url);
        // TODO: Implement download
    } else {
        println!("⚠️  No download URL specified");
    }

    Ok(())
}

fn cmd_firmware_validate(target: &str) -> Result<()> {
    println!("🔍 Validating firmware: {}", target);

    let path = if Path::new(target).exists() {
        PathBuf::from(target)
    } else {
        firmware_dir().join(target).join("dnx_fwr.bin")
    };

    if !path.exists() {
        anyhow::bail!("Firmware file not found: {}", path.display());
    }

    // Use unified API
    let analysis = dnx_core::FirmwareAnalysis::analyze(&path)?;

    println!("  File: {}", analysis.filename);
    println!("  Size: {} bytes", analysis.size);
    println!("  Type: {}", analysis.file_type);
    println!();
    println!("  Validation checks:");

    for check in &analysis.validations {
        let status = if check.passed { "✅" } else { "❌" };
        println!("    {} {}: {}", status, check.name, check.message);
    }

    if analysis.is_valid() {
        println!("\n✅ Firmware validation passed");
    } else {
        println!("\n⚠️  Some validation checks failed");
    }

    Ok(())
}

fn cmd_firmware_extract(source: &Path, output: Option<PathBuf>, component: &str) -> Result<()> {
    println!("📤 Extracting firmware components...");
    println!("  Source: {}", source.display());

    let output_dir = output.unwrap_or_else(|| {
        let mut path = source.to_path_buf();
        path.set_extension("");
        path.with_extension("extracted")
    });

    std::fs::create_dir_all(&output_dir)?;
    println!("  Output: {}", output_dir.display());

    let data = std::fs::read(source)?;

    // Find markers
    let find_marker = |pattern: &[u8]| -> Option<usize> {
        data.windows(pattern.len()).position(|w| w == pattern)
    };

    let cht_pos = find_marker(b"$CHT");
    let ch00_pos = find_marker(b"CH00");
    let cdph_pos = find_marker(b"CDPH");

    let extract_all = component == "all";

    if component == "token" || extract_all {
        if let (Some(cht), Some(ch00)) = (cht_pos, ch00_pos) {
            let start = cht.saturating_sub(0x80);
            let end = ch00.saturating_sub(0x80);
            let token_data = &data[start..end];
            let path = output_dir.join("token.bin");
            std::fs::write(&path, token_data)?;
            println!("  ✅ Extracted token: {} bytes", token_data.len());
        }
    }

    if component == "chaabi" || extract_all {
        if let (Some(ch00), Some(cdph)) = (ch00_pos, cdph_pos) {
            let start = ch00.saturating_sub(0x80);
            let chaabi_data = &data[start..cdph];
            let path = output_dir.join("chaabi.bin");
            std::fs::write(&path, chaabi_data)?;
            println!("  ✅ Extracted chaabi: {} bytes", chaabi_data.len());
        }
    }

    if component == "ifwi" || extract_all {
        if let Some(cht) = cht_pos {
            let end = cht.saturating_sub(0x80);
            let ifwi_data = &data[..end];
            let path = output_dir.join("ifwi.bin");
            std::fs::write(&path, ifwi_data)?;
            println!("  ✅ Extracted ifwi: {} bytes", ifwi_data.len());
        }
    }

    if component == "header" || extract_all {
        let header = &data[..0x188.min(data.len())];
        let path = output_dir.join("header.bin");
        std::fs::write(&path, header)?;
        println!("  ✅ Extracted header: {} bytes", header.len());
    }

    if !extract_all && !matches!(component, "token" | "chaabi" | "ifwi" | "header") {
        println!("  ⚠️  Unknown component: {}", component);
    }

    println!("\n✅ Extraction complete");
    Ok(())
}

fn cmd_firmware_compare(file1: &Path, file2: &Path, detailed: bool) -> Result<()> {
    println!("🔄 Comparing firmware files...");

    let result = dnx_core::FirmwareComparison::compare(file1, file2)?;
    println!("{}", result.to_text());

    if detailed && !result.diff_regions.is_empty() {
        // Detailed binary diff if requested
        if result.diff_count > 0 && result.diff_count < 1000 {
            let data1 = std::fs::read(file1)?;
            let data2 = std::fs::read(file2)?;
            println!("\n  Detailed differences (first 50):");
            let mut shown = 0;
            for (i, (a, b)) in data1.iter().zip(data2.iter()).enumerate() {
                if a != b {
                    println!("    0x{:05X}: {:02X} -> {:02X}", i, a, b);
                    shown += 1;
                    if shown >= 50 {
                        println!("    ... ({} more differences)", result.diff_count - 50);
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}
