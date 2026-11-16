use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use directories::ProjectDirs;

// Deliberately no Debug implementation: the API hash is a credential.
pub struct Config {
    pub(crate) api_id: i32,
    pub(crate) api_hash: String,
    pub(crate) data_dir: PathBuf,
    pub(crate) proxy: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let id = std::env::var("TELEGRAM_API_ID").unwrap_or_default();
        let hash = std::env::var("TELEGRAM_API_HASH").unwrap_or_default();
        let data_dir = match std::env::var_os("SLOPOGRAM_DATA_DIR") {
            Some(path) if !path.is_empty() => PathBuf::from(path),
            _ => ProjectDirs::from("one", "slopogram", "Slopogram")
                .context("Cannot locate application data. Set SLOPOGRAM_DATA_DIR.")?
                .data_local_dir()
                .to_owned(),
        };
        let proxy = std::env::var("SLOPOGRAM_PROXY").unwrap_or_default();
        Self::new(&id, &hash, data_dir)?.with_proxy(if proxy.trim().is_empty() {
            None
        } else {
            Some(proxy.trim().to_owned())
        })
    }

    pub fn new(id: &str, hash: &str, data_dir: PathBuf) -> Result<Self> {
        let api_id = id
            .trim()
            .parse::<i32>()
            .ok()
            .filter(|id| *id > 0)
            .context("Set TELEGRAM_API_ID to your application ID in .env, then restart.")?;
        let api_hash = hash.trim();
        if api_hash.len() != 32 || !api_hash.bytes().all(|c| c.is_ascii_hexdigit()) {
            bail!(
                "Set TELEGRAM_API_HASH to your application's 32-character hash in .env, then restart."
            );
        }
        if data_dir.as_os_str().is_empty() {
            bail!("The session data directory cannot be empty.");
        }
        Ok(Self {
            api_id,
            api_hash: api_hash.into(),
            data_dir,
            proxy: None,
        })
    }

    /// Set the SOCKS5 proxy used for Telegram connections. An invalid URL is
    /// a visible configuration error, never silently ignored.
    pub fn with_proxy(mut self, proxy: Option<String>) -> Result<Self> {
        if let Some(proxy) = proxy {
            validate_proxy(&proxy)?;
            self.proxy = Some(proxy);
        }
        Ok(self)
    }

    pub fn proxy(&self) -> Option<&str> {
        self.proxy.as_deref()
    }
}

fn validate_proxy(proxy: &str) -> Result<()> {
    let rest = proxy
        .strip_prefix("socks5://")
        .context("SLOPOGRAM_PROXY must be a socks5:// URL with a host and port.")?;
    let host_port = rest
        .rsplit_once(':')
        .and_then(|(host, port)| port.parse::<u16>().ok().map(|_| (host, port)));
    match host_port {
        Some((host, _)) if !host.is_empty() => Ok(()),
        _ => bail!("SLOPOGRAM_PROXY must be a socks5:// URL with a host and port."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_credentials_without_echoing_them_in_errors() {
        let secret = "never-print-this-secret";
        let error = Config::new("12", secret, "/tmp/test".into()).err().unwrap();
        assert!(!error.to_string().contains(secret));
        assert!(Config::new("0", &"a".repeat(32), "/tmp/test".into()).is_err());
        assert!(Config::new("12", &"a".repeat(32), "/tmp/test".into()).is_ok());
    }

    #[test]
    fn validates_proxy_urls() {
        let config = Config::new("12", &"a".repeat(32), "/tmp/test".into()).unwrap();
        let with = |proxy: &str| {
            Config::new("12", &"a".repeat(32), "/tmp/test".into())
                .unwrap()
                .with_proxy(Some(proxy.into()))
        };
        assert!(config.proxy().is_none());
        assert_eq!(
            with("socks5://127.0.0.1:1080").unwrap().proxy(),
            Some("socks5://127.0.0.1:1080")
        );
        assert_eq!(
            with("socks5://[::1]:9050").unwrap().proxy(),
            Some("socks5://[::1]:9050")
        );
        assert!(with("http://localhost:8080").is_err());
        assert!(with("socks5://no-port").is_err());
        assert!(with("socks5://:1080").is_err());
        assert!(with("socks5://host:not-a-port").is_err());
    }
}
