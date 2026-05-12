use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use iroh::{EndpointId, SecretKey};
use serde::Deserialize;
use std::str::FromStr;

use crate::default_config_path;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default)]
    pub clients: BTreeMap<String, ClientConfig>,
    #[serde(default)]
    pub exports: BTreeMap<String, ExportConfig>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClientConfig {
    #[serde(skip)]
    pub identity: PathBuf,
    #[serde(default)]
    pub imports: BTreeMap<String, ImportConfig>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImportConfig {
    pub ticket: String,
    pub endpoint_id: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExportConfig {
    #[serde(skip)]
    pub identity: PathBuf,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub allow_clients: Vec<String>,
}

impl Config {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let path = path.map(PathBuf::from).unwrap_or_else(default_config_path);
        let content =
            fs::read_to_string(&path).with_context(|| format!("read config {}", path.display()))?;
        let mut config: Self = serde_json::from_str(&content)
            .with_context(|| format!("parse config {}", path.display()))?;
        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
        config.resolve_identity_paths(base_dir);
        config.validate()?;
        Ok(config)
    }

    pub fn client(&self, name: &str) -> Result<ClientConfig> {
        self.clients
            .get(name)
            .cloned()
            .with_context(|| format!("client profile {name:?} not found"))
    }

    pub fn export(&self, name: &str) -> Result<ExportConfig> {
        self.exports
            .get(name)
            .cloned()
            .with_context(|| format!("export profile {name:?} not found"))
    }

    fn resolve_identity_paths(&mut self, base_dir: &Path) {
        for (name, client) in &mut self.clients {
            client.identity = identity_path(base_dir, IdentityRole::Client, name);
        }
        for (name, export) in &mut self.exports {
            export.identity = identity_path(base_dir, IdentityRole::Export, name);
            if let Some(cwd) = &export.cwd
                && cwd.is_relative()
            {
                export.cwd = Some(base_dir.join(cwd));
            }
        }
    }

    fn validate(&self) -> Result<()> {
        let mut identity_paths = HashSet::new();
        for (name, client) in &self.clients {
            validate_alias("client profile", name)?;
            validate_identity("client", name, &client.identity, &mut identity_paths)?;
            for (alias, import) in &client.imports {
                validate_alias("import", alias)?;
                if import.ticket.is_empty() {
                    bail!("client {name:?} import {alias:?} ticket cannot be empty");
                }
                validate_endpoint_id(
                    &import.endpoint_id,
                    &format!("client {name:?} import {alias:?} endpointId"),
                )?;
            }
        }
        for (name, export) in &self.exports {
            validate_alias("export profile", name)?;
            validate_identity("export", name, &export.identity, &mut identity_paths)?;
            if export.command.is_empty() {
                bail!("export {name:?} command cannot be empty");
            }
            if export.allow_clients.is_empty() {
                bail!("export {name:?} allowClients cannot be empty");
            }
            for client in &export.allow_clients {
                validate_endpoint_id(client, &format!("export {name:?} allowClients entry"))?;
            }
        }
        self.validate_existing_identity_public_keys()?;
        Ok(())
    }

    fn validate_existing_identity_public_keys(&self) -> Result<()> {
        let mut public_keys = BTreeMap::new();
        for (name, client) in &self.clients {
            validate_existing_identity_public_key(
                "client",
                name,
                &client.identity,
                &mut public_keys,
            )?;
        }
        for (name, export) in &self.exports {
            validate_existing_identity_public_key(
                "export",
                name,
                &export.identity,
                &mut public_keys,
            )?;
        }
        Ok(())
    }
}

fn validate_identity(
    role: &str,
    name: &str,
    identity: &Path,
    identity_paths: &mut HashSet<PathBuf>,
) -> Result<()> {
    let comparable = normalize_path_for_compare(identity);
    if !identity_paths.insert(comparable) {
        bail!("{role} {name:?} reuses an identity path");
    }
    Ok(())
}

fn normalize_path_for_compare(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn validate_existing_identity_public_key(
    role: &str,
    name: &str,
    identity: &Path,
    public_keys: &mut BTreeMap<String, String>,
) -> Result<()> {
    if !identity.exists() {
        return Ok(());
    }
    let secret = fs::read_to_string(identity)
        .with_context(|| format!("read identity key {}", identity.display()))?;
    let key = SecretKey::from_str(secret.trim())
        .with_context(|| format!("parse identity key {}", identity.display()))?;
    let public = key.public().to_string();
    let owner = format!("{role} {name:?}");
    if let Some(existing) = public_keys.insert(public, owner.clone()) {
        bail!("{owner} reuses endpoint identity already used by {existing}");
    }
    Ok(())
}

pub fn validate_alias(kind: &str, alias: &str) -> Result<()> {
    if alias.is_empty() {
        bail!("{kind} alias cannot be empty");
    }
    if !alias
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("{kind} alias {alias:?} must be ascii alphanumeric, '-' or '_'");
    }
    if alias.contains("__") {
        bail!("{kind} alias {alias:?} cannot contain reserved delimiter '__'");
    }
    Ok(())
}

fn validate_endpoint_id(value: &str, kind: &str) -> Result<()> {
    if value.is_empty() {
        bail!("{kind} cannot be empty");
    }
    let parsed = EndpointId::from_str(value).with_context(|| format!("{kind} is invalid"))?;
    let canonical = parsed.to_string();
    if value != canonical {
        bail!("{kind} must be canonical endpoint ID {canonical:?}");
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub enum IdentityRole {
    Client,
    Export,
}

impl IdentityRole {
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::Client => "clients",
            Self::Export => "exports",
        }
    }

    pub fn command_name(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Export => "export",
        }
    }
}

pub fn config_base_dir(path: Option<&Path>) -> PathBuf {
    let path = path.map(PathBuf::from).unwrap_or_else(default_config_path);
    path.parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

pub fn identity_path(base_dir: &Path, role: IdentityRole, profile: &str) -> PathBuf {
    base_dir
        .join("identities")
        .join(role.dir_name())
        .join(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_config(input: &str) -> Result<Config> {
        let mut config: Config = serde_json::from_str(input)?;
        config.resolve_identity_paths(Path::new("/home/me/.config/tool-tunnel"));
        config.validate()?;
        Ok(config)
    }

    fn endpoint_id() -> String {
        SecretKey::generate().public().to_string()
    }

    #[test]
    fn parses_client_imports_and_export_profiles() -> Result<()> {
        let endpoint = endpoint_id();
        let input = r#"
        {
          "clients": {
            "default": {
              "imports": {
                "mini": {
                  "ticket": "ticket",
                  "endpointId": "$ENDPOINT"
                }
              }
            }
          },
          "exports": {
            "mini-tools": {
              "command": "uv",
              "args": ["run", "server.py"],
              "cwd": "tools",
              "env": {
                "CACHE_DIR": "/tmp/tool-tunnel"
              },
              "allowClients": ["$ENDPOINT"]
            }
          }
        }
        "#
        .replace("$ENDPOINT", &endpoint);
        let config = parse_config(&input)?;
        assert_eq!(
            config.client("default")?.identity,
            PathBuf::from("/home/me/.config/tool-tunnel/identities/clients/default")
        );
        assert_eq!(
            config.export("mini-tools")?.cwd,
            Some(PathBuf::from("/home/me/.config/tool-tunnel/tools"))
        );
        Ok(())
    }

    #[test]
    fn derives_distinct_identities_for_matching_client_and_export_names() -> Result<()> {
        let endpoint = endpoint_id();
        let input = r#"
        {
          "clients": {
            "default": {
              "imports": {}
            }
          },
          "exports": {
            "default": {
              "command": "server",
              "allowClients": ["$ENDPOINT"]
            }
          }
        }
        "#
        .replace("$ENDPOINT", &endpoint);
        let config = parse_config(&input)?;

        assert_eq!(
            config.client("default")?.identity,
            PathBuf::from("/home/me/.config/tool-tunnel/identities/clients/default")
        );
        assert_eq!(
            config.export("default")?.identity,
            PathBuf::from("/home/me/.config/tool-tunnel/identities/exports/default")
        );
        Ok(())
    }

    #[test]
    fn rejects_alias_with_reserved_delimiter() {
        let result = parse_config(
            r#"
            {
              "clients": {
                "lab__mini": {
                  "imports": {}
                }
              },
              "exports": {}
            }
            "#,
        );
        let message = match result {
            Ok(_) => String::new(),
            Err(error) => error.to_string(),
        };

        assert!(message.contains("reserved delimiter"));
    }

    #[test]
    fn rejects_export_without_allow_clients() {
        let result = parse_config(
            r#"
            {
              "clients": {},
              "exports": {
                "mini-tools": {
                  "command": "server"
                }
              }
            }
            "#,
        );
        let message = match result {
            Ok(_) => String::new(),
            Err(error) => error.to_string(),
        };

        assert!(message.contains("allowClients cannot be empty"));
    }

    #[test]
    fn rejects_invalid_endpoint_id() {
        let result = parse_config(
            r#"
            {
              "clients": {
                "default": {
                  "imports": {
                    "mini": {
                      "ticket": "ticket",
                      "endpointId": "not-an-endpoint"
                    }
                  }
                }
              },
              "exports": {}
            }
            "#,
        );
        let message = match result {
            Ok(_) => String::new(),
            Err(error) => error.to_string(),
        };

        assert!(message.contains("endpointId is invalid"));
    }
}
