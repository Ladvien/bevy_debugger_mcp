/// Test game fixtures for E2E screenshot testing
/// 
/// This module provides predictable Bevy game instances for testing
/// the screenshot functionality in various scenarios.

pub mod static_test_game;
pub mod animated_test_game;

pub use static_test_game::run_static_test_game;
pub use animated_test_game::run_animated_test_game;

use std::process::{Child, Command};
use std::time::Duration;
use tokio::time::timeout;

/// Configuration for test game instances
#[derive(Debug, Clone)]
pub struct TestGameConfig {
    /// Port for the BRP server
    pub brp_port: u16,
    /// Window width
    pub width: u32,
    /// Window height
    pub height: u32,
    /// Maximum time to wait for game startup
    pub startup_timeout: Duration,
    /// Whether to run headless (for CI)
    pub headless: bool,
}

impl Default for TestGameConfig {
    fn default() -> Self {
        Self {
            brp_port: 15702,
            width: 800,
            height: 600,
            startup_timeout: Duration::from_secs(10),
            headless: std::env::var("CI").is_ok(), // Auto-detect CI environment
        }
    }
}

/// Test game launcher for E2E tests
pub struct TestGameLauncher {
    config: TestGameConfig,
}

impl TestGameLauncher {
    pub fn new(config: TestGameConfig) -> Self {
        Self { config }
    }

    /// Launch a static test game instance
    pub async fn launch_static_game(&self) -> Result<TestGameInstance, Box<dyn std::error::Error>> {
        self.launch_game_binary("static_test_game").await
    }

    /// Launch an animated test game instance
    pub async fn launch_animated_game(&self) -> Result<TestGameInstance, Box<dyn std::error::Error>> {
        self.launch_game_binary("animated_test_game").await
    }

    async fn launch_game_binary(&self, game_type: &str) -> Result<TestGameInstance, Box<dyn std::error::Error>> {
        let mut cmd = Command::new("cargo");
        cmd.args(&["run", "--bin", &format!("test_{}", game_type)])
            .env("BEVY_BRP_PORT", self.config.brp_port.to_string())
            .env("BEVY_WINDOW_WIDTH", self.config.width.to_string())
            .env("BEVY_WINDOW_HEIGHT", self.config.height.to_string());

        if self.config.headless {
            cmd.env("BEVY_HEADLESS", "1");
        }

        let child = cmd.spawn()?;

        // Wait for the game to start up and begin accepting BRP connections
        let startup_result = timeout(
            self.config.startup_timeout,
            self.wait_for_brp_connection()
        ).await;

        match startup_result {
            Ok(Ok(())) => {
                Ok(TestGameInstance {
                    process: Some(child),
                    brp_port: self.config.brp_port,
                    config: self.config.clone(),
                })
            }
            Ok(Err(e)) => {
                // Kill the process if it started but BRP connection failed
                if let Ok(mut child) = Command::new("pkill")
                    .arg("-f")
                    .arg(&format!("test_{}", game_type))
                    .spawn()
                {
                    let _ = child.wait();
                }
                Err(e)
            }
            Err(_) => {
                Err("Game startup timeout".into())
            }
        }
    }

    /// Wait for BRP connection to be available
    async fn wait_for_brp_connection(&self) -> Result<(), Box<dyn std::error::Error>> {
        use tokio::net::TcpStream;
        use tokio::time::{sleep, Duration};

        let max_attempts = 50;
        let retry_delay = Duration::from_millis(200);

        for attempt in 1..=max_attempts {
            match TcpStream::connect(format!("127.0.0.1:{}", self.config.brp_port)).await {
                Ok(_) => {
                    println!("BRP connection established on attempt {}", attempt);
                    return Ok(());
                }
                Err(_) => {
                    if attempt < max_attempts {
                        sleep(retry_delay).await;
                    }
                }
            }
        }

        Err("Failed to establish BRP connection".into())
    }
}

/// A running test game instance
pub struct TestGameInstance {
    process: Option<Child>,
    pub brp_port: u16,
    pub config: TestGameConfig,
}

impl TestGameInstance {
    /// Get the BRP connection URL
    pub fn brp_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.brp_port)
    }

    /// Gracefully shutdown the game
    pub fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(mut child) = self.process.take() {
            // Try graceful shutdown first
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                use nix::sys::signal::{self, Signal};
                use nix::unistd::Pid;

                let pid = Pid::from_raw(child.id() as i32);
                if let Err(_) = signal::kill(pid, Signal::SIGTERM) {
                    // Fallback to force kill
                    let _ = child.kill();
                }
            }

            #[cfg(not(unix))]
            {
                let _ = child.kill();
            }

            let _ = child.wait();
        }
        Ok(())
    }
}

impl Drop for TestGameInstance {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_game_launcher_config() {
        let config = TestGameConfig::default();
        assert_eq!(config.brp_port, 15702);
        assert_eq!(config.width, 800);
        assert_eq!(config.height, 600);
    }

    #[test]
    fn test_game_instance_url() {
        let config = TestGameConfig::default();
        let instance = TestGameInstance {
            process: None,
            brp_port: 15702,
            config,
        };
        assert_eq!(instance.brp_url(), "http://127.0.0.1:15702");
    }
}