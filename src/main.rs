use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tokio::fs;

#[derive(Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
enum CargoCli {
    Pumpkin(PumpkinArgs),
}

#[derive(Parser)]
#[command(version, about = "Build and run your Pumpkin plugin")]
struct PumpkinArgs {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Force rebuild of Pumpkin even if it exists
    #[arg(short, long)]
    force: bool,

    /// Skip building the current project
    #[arg(long)]
    skip_self_build: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize and setup the environment
    Init,
    /// Build and run the server
    Run,
    /// Clean the .run directory
    Clean,
}

#[tokio::main]
async fn main() -> Result<()> {
    let CargoCli::Pumpkin(args) = CargoCli::parse();

    let pumpkin_runner = PumpkinRunner::new().await?;

    match args.command.unwrap_or(Commands::Run) {
        Commands::Init => pumpkin_runner.init(args.force).await,
        Commands::Run => pumpkin_runner.run(args.force, args.skip_self_build).await,
        Commands::Clean => pumpkin_runner.clean().await,
    }
}

struct PumpkinRunner {
    current_dir: PathBuf,
    run_dir: PathBuf,
    pumpkin_dir: PathBuf,
}

impl PumpkinRunner {
    async fn new() -> Result<Self> {
        let current_dir = std::env::current_dir().context("Failed to get current directory")?;

        let run_dir = current_dir.join(".run");
        let pumpkin_dir = current_dir.join("Pumpkin");

        Ok(Self {
            current_dir,
            run_dir,
            pumpkin_dir,
        })
    }

    async fn init(&self, force: bool) -> Result<()> {
        println!("{}", "Initializing Pumpkin environment...".yellow().bold());

        fs::create_dir_all(&self.run_dir)
            .await
            .context("Failed to create .run directory")?;

        self.setup_pumpkin_repo(force).await?;

        println!("{}", "Initialization complete!".green().bold());
        Ok(())
    }

    async fn run(&self, force: bool, skip_self_build: bool) -> Result<()> {
        println!("{}", "Starting Pumpkin runner...".yellow().bold());

        fs::create_dir_all(&self.run_dir)
            .await
            .context("Failed to create .run directory")?;

        if force || !self.pumpkin_dir.exists() {
            self.setup_pumpkin_repo(force).await?;
        }

        if !skip_self_build {
            self.build_current_project().await?;
        }

        self.build_pumpkin_server().await?;

        self.copy_artifacts().await?;

        self.run_server().await?;

        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        println!("{}", "Cleaning .run directory...".yellow().bold());

        if self.run_dir.exists() {
            fs::remove_dir_all(&self.run_dir)
                .await
                .context("Failed to remove .run directory")?;
        }

        println!("{}", "Clean complete!".green().bold());
        Ok(())
    }

    async fn setup_pumpkin_repo(&self, force: bool) -> Result<()> {
        if self.pumpkin_dir.exists() {
            if force {
                println!("{}", "Force rebuilding Pumpkin...".blue());
                fs::remove_dir_all(&self.pumpkin_dir)
                    .await
                    .context("Failed to remove existing Pumpkin directory")?;
            } else {
                println!(
                    "{}",
                    "Pumpkin repository already exists, pulling latest changes...".blue()
                );
                self.git_pull().await?;
                return Ok(());
            }
        }

        println!("{}", "Cloning Pumpkin repository...".blue());

        let output = Command::new("git")
            .args(&["clone", "https://github.com/Pumpkin-MC/Pumpkin.git"])
            .current_dir(&self.current_dir)
            .output()
            .context("Failed to execute git clone")?;

        if !output.status.success() {
            anyhow::bail!(
                "Git clone failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        println!("{}", "Pumpkin repository cloned successfully!".green());
        Ok(())
    }

    async fn git_pull(&self) -> Result<()> {
        let output = Command::new("git")
            .args(&["pull"])
            .current_dir(&self.pumpkin_dir)
            .output()
            .context("Failed to execute git pull")?;

        if !output.status.success() {
            println!(
                "{}",
                "Git pull failed, continuing with existing version...".yellow()
            );
        } else {
            println!("{}", "Pumpkin repository updated!".green());
        }

        Ok(())
    }

    async fn build_current_project(&self) -> Result<()> {
        println!("{}", "Building current project...".blue());

        let mut args = vec!["build"];

        if cfg!(target_os = "windows") {
            args.push("--release");
            println!(
                "{}",
                "  Windows detected: Using release build for plugin compatibility".yellow()
            );
        }

        let output = Command::new("cargo")
            .args(&args)
            .current_dir(&self.current_dir)
            .output()
            .context("Failed to build current project")?;

        if !output.status.success() {
            anyhow::bail!(
                "Current plugin build failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        println!("{}", "Plugin built successfully!".green());
        Ok(())
    }

    async fn build_pumpkin_server(&self) -> Result<()> {
        println!("{}", "Building Pumpkin server...".blue());

        let output = Command::new("cargo")
            .args(&["build"])
            .current_dir(&self.pumpkin_dir)
            .output()
            .context("Failed to build Pumpkin server")?;

        if !output.status.success() {
            anyhow::bail!(
                "Pumpkin server build failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        println!("{}", "Pumpkin server built successfully!".green());
        Ok(())
    }

    async fn copy_artifacts(&self) -> Result<()> {
        println!("{}", "Copying artifacts to .run directory...".blue());

        let pumpkin_binary = self.pumpkin_dir.join("target/debug/pumpkin");
        if pumpkin_binary.exists() {
            let dest = self.run_dir.join("pumpkin");
            fs::copy(&pumpkin_binary, &dest)
                .await
                .context("Failed to copy Pumpkin binary")?;
            println!("{}", "  Copied Pumpkin server binary".green());
        }

        let project_name = self.get_project_name().await?;

        if let Some(name) = project_name {
            self.copy_plugin_artifact(&name).await?;
        }

        println!("{}", "Artifacts copied successfully!".green());
        Ok(())
    }

    async fn get_project_name(&self) -> Result<Option<String>> {
        let cargo_toml_path = self.current_dir.join("Cargo.toml");
        if !cargo_toml_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&cargo_toml_path)
            .await
            .context("Failed to read Cargo.toml")?;

        for line in content.lines() {
            if line.trim().starts_with("name") && line.contains("=") {
                let name = line
                    .split('=')
                    .nth(1)
                    .unwrap_or("")
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                return Ok(Some(name.to_string()));
            }
        }

        Ok(None)
    }

    async fn copy_plugin_artifact(&self, name: &str) -> Result<()> {
        let build_dir = if cfg!(target_os = "windows") {
            "release"
        } else {
            "debug"
        };

        let (lib_prefix, extension) = if cfg!(target_os = "windows") {
            ("", ".dll")
        } else if cfg!(target_os = "macos") {
            ("lib", ".dylib")
        } else {
            ("lib", ".so")
        };

        let plugin_filename = format!("{}{}{}", lib_prefix, name.replace("-", "_"), extension);
        let plugin_path = self
            .current_dir
            .join(format!("target/{}/{}", build_dir, plugin_filename));

        if plugin_path.exists() {
            let plugins_dir = self.run_dir.join("plugins");
            fs::create_dir_all(&plugins_dir)
                .await
                .context("Failed to create plugins directory")?;

            let dest = plugins_dir.join(&plugin_filename);
            fs::copy(&plugin_path, &dest)
                .await
                .context("Failed to copy plugin file")?;
            println!(
                "{}",
                format!("  Copied plugin {} to plugins/", plugin_filename).green()
            );
        } else {
            println!(
                "{}",
                format!(
                    "  Plugin {} not found at {}",
                    plugin_filename,
                    plugin_path.display()
                )
                .yellow()
            );
        }

        Ok(())
    }

    async fn run_server(&self) -> Result<()> {
        println!("{}", "Starting Pumpkin server...".yellow().bold());

        let pumpkin_binary = self.run_dir.join("pumpkin");

        if !pumpkin_binary.exists() {
            anyhow::bail!("Pumpkin binary not found in .run directory");
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&pumpkin_binary).await?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&pumpkin_binary, perms).await?;
        }

        println!(
            "{}",
            "Server is starting... (Press Ctrl+C to stop)"
                .green()
                .bold()
        );

        let mut child = Command::new(&pumpkin_binary)
            .current_dir(&self.run_dir)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .context("Failed to start Pumpkin server")?;

        let status = child.wait().context("Failed to wait for server process")?;

        if status.success() {
            println!("{}", "Server stopped successfully".green());
        } else {
            println!("{}", "Server stopped with error".red());
        }

        Ok(())
    }
}
