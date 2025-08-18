use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use colored::*;
use serde_json::json;

#[derive(Parser)]
#[command(name = "bevy-debugger-mcp")]
#[command(author = "ladvien")]
#[command(version = "0.1.0")]
#[command(about = "Debug Bevy games with Claude AI", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
    
    /// Configuration file path
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,
    
    /// Log level (error, warn, info, debug, trace)
    #[arg(short, long, global = true, default_value = "info")]
    pub log_level: String,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the MCP server (default)
    Serve {
        /// Host where Bevy game is running
        #[arg(long, env = "BEVY_BRP_HOST", default_value = "localhost")]
        bevy_host: String,
        
        /// Port of Bevy Remote Protocol
        #[arg(long, env = "BEVY_BRP_PORT", default_value = "15702")]
        bevy_port: u16,
        
        /// Port for MCP server to listen on
        #[arg(long, env = "MCP_PORT", default_value = "3000")]
        mcp_port: u16,
    },
    
    /// Run diagnostic checks
    Doctor,
    
    /// Test connection to Bevy game
    Test {
        /// Host to test
        #[arg(long, default_value = "localhost")]
        host: String,
        
        /// Port to test
        #[arg(long, default_value = "15702")]
        port: u16,
    },
    
    /// Set up Claude Desktop integration
    SetupClaude {
        /// Force overwrite existing configuration
        #[arg(short, long)]
        force: bool,
    },
    
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    
    /// Initialize a new Bevy project with debugging support
    Init {
        /// Project name
        name: String,
        
        /// Project template
        #[arg(long, default_value = "basic")]
        template: String,
    },
    
    /// Export diagnostic report
    Diagnose {
        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,
        
        /// Include sensitive information
        #[arg(long)]
        include_sensitive: bool,
    },
    
    /// Manage recordings
    Recording {
        #[command(subcommand)]
        action: RecordingCommands,
    },
    
    /// Manage checkpoints
    Checkpoint {
        #[command(subcommand)]
        action: CheckpointCommands,
    },
}

#[derive(Subcommand)]
pub enum RecordingCommands {
    /// List all recordings
    List,
    
    /// Play a recording
    Play {
        /// Recording ID or name
        id: String,
        
        /// Playback speed
        #[arg(long, default_value = "1.0")]
        speed: f32,
    },
    
    /// Delete a recording
    Delete {
        /// Recording ID or name
        id: String,
    },
    
    /// Export recording to file
    Export {
        /// Recording ID
        id: String,
        
        /// Output file
        output: PathBuf,
    },
}

#[derive(Subcommand)]
pub enum CheckpointCommands {
    /// List all checkpoints
    List,
    
    /// Create a new checkpoint
    Create {
        /// Checkpoint name
        name: String,
    },
    
    /// Restore from checkpoint
    Restore {
        /// Checkpoint ID or name
        id: String,
    },
    
    /// Delete a checkpoint
    Delete {
        /// Checkpoint ID or name
        id: String,
    },
}

pub fn run_doctor() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "🔍 Running diagnostic checks...".blue().bold());
    println!();
    
    let mut checks_passed = true;
    
    // Check 1: Rust version
    print!("Checking Rust version... ");
    match Command::new("rustc").arg("--version").output() {
        Ok(output) => {
            let version = String::from_utf8_lossy(&output.stdout);
            println!("{} {}", "✓".green(), version.trim());
        }
        Err(_) => {
            println!("{} Rust not found", "✗".red());
            checks_passed = false;
        }
    }
    
    // Check 2: Config file
    print!("Checking configuration... ");
    let config_path = dirs::config_dir()
        .map(|p| p.join("bevy-debugger").join("config.toml"))
        .unwrap_or_default();
    
    if config_path.exists() {
        println!("{} Found at {:?}", "✓".green(), config_path);
    } else {
        println!("{} Not found (will use defaults)", "⚠".yellow());
    }
    
    // Check 3: Claude Desktop
    print!("Checking Claude Desktop... ");
    let claude_config = if cfg!(target_os = "macos") {
        dirs::home_dir()
            .map(|p| p.join("Library/Application Support/Claude/claude_desktop_config.json"))
    } else {
        dirs::config_dir()
            .map(|p| p.join("Claude/claude_desktop_config.json"))
    };
    
    if let Some(path) = claude_config {
        if path.exists() {
            // Check if our server is configured
            if let Ok(content) = fs::read_to_string(&path) {
                if content.contains("bevy-debugger") {
                    println!("{} Configured", "✓".green());
                } else {
                    println!("{} Not configured (run 'setup-claude')", "⚠".yellow());
                }
            } else {
                println!("{} Can't read config", "⚠".yellow());
            }
        } else {
            println!("{} Not installed", "✗".red());
            checks_passed = false;
        }
    }
    
    // Check 4: Network connectivity
    print!("Checking network... ");
    match std::net::TcpStream::connect("127.0.0.1:15702") {
        Ok(_) => println!("{} Bevy game detected on port 15702", "✓".green()),
        Err(_) => println!("{} No Bevy game running (expected)", "⚠".yellow()),
    }
    
    // Check 5: Port availability
    print!("Checking MCP port 3000... ");
    match std::net::TcpListener::bind("127.0.0.1:3000") {
        Ok(_) => println!("{} Available", "✓".green()),
        Err(_) => {
            println!("{} In use (may need to stop existing server)", "⚠".yellow());
        }
    }
    
    println!();
    if checks_passed {
        println!("{}", "✅ All critical checks passed!".green().bold());
        println!();
        println!("To start debugging:");
        println!("  1. Start your Bevy game with RemotePlugin");
        println!("  2. Run: bevy-debugger-mcp serve");
        println!("  3. Open Claude Desktop");
    } else {
        println!("{}", "❌ Some checks failed. Please fix the issues above.".red().bold());
    }
    
    Ok(())
}

pub fn setup_claude(force: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "🔧 Setting up Claude Desktop integration...".blue().bold());
    
    let claude_config_path = if cfg!(target_os = "macos") {
        dirs::home_dir()
            .map(|p| p.join("Library/Application Support/Claude/claude_desktop_config.json"))
            .ok_or("Could not find home directory")?
    } else {
        dirs::config_dir()
            .map(|p| p.join("Claude/claude_desktop_config.json"))
            .ok_or("Could not find config directory")?
    };
    
    // Check if Claude is installed
    if !claude_config_path.parent().unwrap().exists() {
        return Err("Claude Desktop not found. Please install it first.".into());
    }
    
    // Backup existing config
    if claude_config_path.exists() && !force {
        let backup_path = claude_config_path.with_extension("json.bak");
        fs::copy(&claude_config_path, &backup_path)?;
        println!("📁 Backed up existing config to {:?}", backup_path);
    }
    
    // Create MCP server configuration
    let config = json!({
        "mcpServers": {
            "bevy-debugger": {
                "command": "bevy-debugger-mcp",
                "args": ["serve"],
                "env": {
                    "RUST_LOG": "info"
                }
            }
        }
    });
    
    // Write configuration
    fs::create_dir_all(claude_config_path.parent().unwrap())?;
    fs::write(&claude_config_path, serde_json::to_string_pretty(&config)?)?;
    
    println!("{} Configuration written to {:?}", "✓".green(), claude_config_path);
    println!();
    println!("{}", "✅ Claude Desktop configured successfully!".green().bold());
    println!();
    println!("Next steps:");
    println!("  1. {} Claude Desktop completely", "Restart".yellow());
    println!("  2. Look for the {} icon in Claude", "🔌 MCP".cyan());
    println!("  3. Start debugging your Bevy game!");
    
    Ok(())
}

pub fn test_connection(host: &str, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", format!("🔌 Testing connection to {}:{}...", host, port).blue().bold());
    
    use std::time::Duration;
    use tokio_tungstenite::connect_async;
    use tokio::time::timeout;
    
    let rt = tokio::runtime::Runtime::new()?;
    
    rt.block_on(async {
        let url = format!("ws://{}:{}", host, port);
        
        match timeout(Duration::from_secs(5), connect_async(&url)).await {
            Ok(Ok((mut ws, _))) => {
                println!("{} Connected successfully!", "✓".green());
                
                // Try to send a simple request
                use tokio_tungstenite::tungstenite::Message;
                let test_msg = json!({
                    "method": "bevy/list",
                    "params": {}
                });
                
                ws.send(Message::Text(test_msg.to_string())).await?;
                println!("{} Sent test message", "✓".green());
                
                if let Some(Ok(response)) = ws.next().await {
                    println!("{} Received response: {:?}", "✓".green(), response);
                }
                
                println!();
                println!("{}", "✅ Connection test successful!".green().bold());
                println!("Your Bevy game is ready for debugging.");
            }
            Ok(Err(e)) => {
                println!("{} Connection failed: {}", "✗".red(), e);
                println!();
                println!("Make sure:");
                println!("  • Your Bevy game is running");
                println!("  • RemotePlugin is added to your app");
                println!("  • The correct port is specified");
            }
            Err(_) => {
                println!("{} Connection timeout", "✗".red());
                println!();
                println!("Could not connect to {}:{}", host, port);
                println!("Is your Bevy game running?");
            }
        }
        
        Ok::<(), Box<dyn std::error::Error>>(())
    })?;
    
    Ok(())
}

pub fn init_project(name: &str, template: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", format!("🚀 Creating new Bevy project '{}'...", name).blue().bold());
    
    // Create project with cargo
    Command::new("cargo")
        .args(&["new", name, "--bin"])
        .status()?;
    
    // Add dependencies to Cargo.toml
    let cargo_toml_path = PathBuf::from(name).join("Cargo.toml");
    let mut cargo_toml = fs::read_to_string(&cargo_toml_path)?;
    
    cargo_toml.push_str(r#"
bevy = { version = "0.14", features = ["remote"] }
bevy-debugger-mcp-helper = "0.1"  # Optional helper crate

[features]
debug = ["bevy/remote"]
"#);
    
    fs::write(&cargo_toml_path, cargo_toml)?;
    
    // Create main.rs with RemotePlugin
    let main_rs = match template {
        "basic" => include_str!("../templates/basic.rs"),
        "3d" => include_str!("../templates/3d.rs"),
        "2d" => include_str!("../templates/2d.rs"),
        _ => include_str!("../templates/basic.rs"),
    };
    
    let main_path = PathBuf::from(name).join("src").join("main.rs");
    fs::write(&main_path, main_rs)?;
    
    // Create .env file
    let env_content = r#"# Bevy Debugger Configuration
BEVY_BRP_HOST=localhost
BEVY_BRP_PORT=15702
RUST_LOG=info
"#;
    fs::write(PathBuf::from(name).join(".env"), env_content)?;
    
    // Create launch script
    let launch_script = r#"#!/bin/bash
# Launch script for debugging
cargo run --features debug &
GAME_PID=$!
sleep 2
bevy-debugger-mcp serve
kill $GAME_PID
"#;
    
    let script_path = PathBuf::from(name).join("debug.sh");
    fs::write(&script_path, launch_script)?;
    
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))?;
    }
    
    println!("{} Project created successfully!", "✓".green());
    println!();
    println!("Next steps:");
    println!("  cd {}", name);
    println!("  ./debug.sh  # Start game with debugger");
    
    Ok(())
}