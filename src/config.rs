use crate::error::{Error, Result};
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub bevy_brp_host: String,
    pub bevy_brp_port: u16,
    pub mcp_port: u16,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let bevy_brp_host = env::var("BEVY_BRP_HOST").unwrap_or_else(|_| "localhost".to_string());
        let bevy_brp_port = env::var("BEVY_BRP_PORT")
            .unwrap_or_else(|_| "15702".to_string())
            .parse::<u16>()
            .map_err(|_| Error::Config("Invalid BEVY_BRP_PORT".to_string()))?;
        let mcp_port = env::var("MCP_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse::<u16>()
            .map_err(|_| Error::Config("Invalid MCP_PORT".to_string()))?;

        Ok(Config {
            bevy_brp_host,
            bevy_brp_port,
            mcp_port,
        })
    }

    #[must_use]
    pub fn brp_url(&self) -> String {
        format!("ws://{}:{}", self.bevy_brp_host, self.bevy_brp_port)
    }
}
