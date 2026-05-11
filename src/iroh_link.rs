use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    str::FromStr,
    time::Duration,
};

use anyhow::{Context, Result};
use data_encoding::HEXLOWER;
use iroh::{Endpoint, SecretKey, endpoint::presets};
use iroh_tickets::endpoint::EndpointTicket;
use tokio::time::timeout;

pub const ALPN: &[u8] = b"tool-tunnel/stdio/0";
pub const HANDSHAKE: &[u8] = b"tool-tunnel\n";

const ONLINE_TIMEOUT: Duration = Duration::from_secs(5);

pub fn secret_from_file(path: &Path) -> Result<SecretKey> {
    let secret = fs::read_to_string(path)
        .with_context(|| format!("read identity key {}", path.display()))?;
    SecretKey::from_str(secret.trim())
        .with_context(|| format!("parse identity key {}", path.display()))
}

pub fn init_identity(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create identity directory {}", parent.display()))?;
    }
    let key = SecretKey::generate();
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("create identity key {}", path.display()))?;
    writeln!(file, "{}", HEXLOWER.encode(&key.to_bytes()))
        .with_context(|| format!("write identity key {}", path.display()))
}

pub fn public_key_from_file(path: &Path) -> Result<String> {
    let key = secret_from_file(path)?;
    Ok(key.public().to_string())
}

pub async fn endpoint(secret: SecretKey, alpns: Vec<Vec<u8>>) -> Result<Endpoint> {
    Endpoint::builder(presets::N0)
        .secret_key(secret)
        .alpns(alpns)
        .bind()
        .await
        .context("bind iroh endpoint")
}

pub async fn wait_online(endpoint: &Endpoint) {
    if timeout(ONLINE_TIMEOUT, endpoint.online()).await.is_err() {
        eprintln!("warning: failed to connect to the home relay before publishing ticket");
    }
}

pub fn ticket(endpoint: &Endpoint) -> EndpointTicket {
    EndpointTicket::new(endpoint.addr())
}

pub fn parse_ticket(ticket: &str) -> Result<EndpointTicket> {
    EndpointTicket::from_str(ticket).context("invalid endpoint ticket")
}
