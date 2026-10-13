//! `[operate]`: the node's operate listener (ADR-0067), where an operator
//! surface on another machine reads, follows and acts on the node over
//! gRPC inside Xmip's mutual TLS (ADR-0014, amendment 2026-10-12).
//!
//! Absent, the node serves no operate listener and is operated on its own
//! machine only. Present, every key but `role` is required:
//!
//! ```toml
//! [operate]
//! listen = "0.0.0.0:7401"             # the node's address, the plane's port
//! certificate = "tls/node.pem"        # this node's chain, leaf first
//! key = "tls/node.key"                # its private key
//! anchors = "tls/cluster-anchor.pem"  # what a surface's certificate reaches
//! role = "operator"                   # every proven caller's; Observer if left out
//! ```
//!
//! A relative path is relative to the configuration file. The address is
//! the node's one address (ADR-0067 clause 1) and the port the operate
//! plane's own; no port is assumed. `role` is the role the run states for
//! every caller a certificate proved (ADR-0009, amendment 2026-10-06): a
//! word that is no role grants Observer, which acts on nothing.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// `[operate]`, as the document says it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperateConfiguration {
    /// Where the listener binds: `host:port`.
    pub listen: String,
    /// This node's PEM certificate chain, leaf first.
    pub certificate: String,
    /// Its PEM private key.
    pub key: String,
    /// The PEM anchors a surface's certificate must reach.
    pub anchors: String,
    /// The role every proven caller holds; Observer where it is left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

impl OperateConfiguration {
    /// `path` resolved against the configuration file at `configuration`.
    #[must_use]
    pub fn resolve(configuration: &Path, path: &str) -> PathBuf {
        let base = configuration.parent().unwrap_or_else(|| Path::new(""));
        base.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_it_does_not_know_is_refused_and_a_path_is_the_files() {
        let read: Result<OperateConfiguration, _> = toml::from_str(
            "listen = \"127.0.0.1:7401\"\ncertificate = \"c\"\nkey = \"k\"\nanchors = \"a\"\n",
        );
        assert_eq!(read.expect("read").role, None);
        let wrong: Result<OperateConfiguration, _> = toml::from_str(
            "listen = \"x:1\"\ncertificate = \"c\"\nkey = \"k\"\nanchors = \"a\"\nport = 1\n",
        );
        assert!(wrong.is_err());
        assert_eq!(
            OperateConfiguration::resolve(Path::new("/opt/xmip/config/node.toml"), "tls/n.pem"),
            Path::new("/opt/xmip/config/tls/n.pem")
        );
    }
}
