//! `[operate]`: the node's operate listener (ADR-0067), where an operator
//! surface reads, follows and acts on the node over gRPC — from another
//! machine on TCP inside Xmip's mutual TLS (ADR-0014, amendment
//! 2026-10-12), and from its own on its local endpoint, a named pipe on
//! Windows and a Unix domain socket elsewhere (ADR-0014, amendment
//! 2026-10-13).
//!
//! The local endpoint always starts — decided by the owner, 2026-10-13:
//! *the local endpoint always starts, named `xmip-<node>` unless the
//! configuration names it* — so a node with no `[operate]` is operated on
//! its own machine at `xmip-<node>` and from nowhere else. `local` names
//! the endpoint otherwise; `listen` adds the network listener and takes
//! `certificate`, `key` and `anchors` with it:
//!
//! ```toml
//! [operate]
//! local = "xmip-node1"                # the local endpoint's name
//! listen = "0.0.0.0:7401"             # the node's address, the plane's port
//! certificate = "tls/node.pem"        # this node's chain, leaf first
//! key = "tls/node.key"                # its private key
//! anchors = "tls/cluster-anchor.pem"  # what a surface's certificate reaches
//! role = "operator"                   # every proven caller's; Observer if left out
//! admit = ['CONTOSO\xmip-operators']  # who else may open the local endpoint
//! ```
//!
//! **Who may open the local endpoint** is the operating system's to
//! enforce and the configuration's to say (the owner, 2026-10-19: *local
//! callers are proven by the operating system*): the node's own account
//! and the machine's administrators — on Unix the superuser — always, and
//! each account or group `admit` names beside them; nobody else. A name
//! the system does not know is refused as the node starts. A tcp
//! connection is never one of them for coming over loopback: it presents
//! a certificate like any caller from another machine.
//!
//! A relative path is relative to the configuration file. The address is
//! the node's one address (ADR-0067 clause 1) and the port the operate
//! plane's own; no port is assumed. A local name is `\\.\pipe\<name>` on
//! Windows, `/run/xmip/<name>.sock` on Linux and
//! `/var/run/xmip/<name>.sock` on macOS; a name may change, the folder may
//! not, and one that would leave it is refused as the node starts
//! (`xmip-core-runtime`'s `plane::local::place`). `role` is the role the
//! run states for every caller the connection proved — by its
//! certificate, or on the local endpoint by the operating system
//! (ADR-0009, amendment 2026-10-06): a word that is no role grants
//! Observer, which acts on nothing.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// `[operate]`, as the document says it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperateConfiguration {
    /// The local endpoint's name; `xmip-<node>` where it is left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<String>,
    /// Where the network listener binds: `host:port`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen: Option<String>,
    /// This node's PEM certificate chain, leaf first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate: Option<String>,
    /// Its PEM private key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// The PEM anchors a surface's certificate must reach.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchors: Option<String>,
    /// The role every proven caller holds; Observer where it is left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// The accounts and groups the operating system lets open the local
    /// endpoint beside the node's own account and the machine's
    /// administrators; nobody else where it is left out.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub admit: Vec<String>,
}

/// The network listener's keys, all given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Network<'a> {
    /// `host:port`.
    pub listen: &'a str,
    /// The certificate chain's file.
    pub certificate: &'a str,
    /// The private key's file.
    pub key: &'a str,
    /// The anchors' file.
    pub anchors: &'a str,
}

impl OperateConfiguration {
    /// `path` resolved against the configuration file at `configuration`.
    #[must_use]
    pub fn resolve(configuration: &Path, path: &str) -> PathBuf {
        let base = configuration.parent().unwrap_or_else(|| Path::new(""));
        base.join(path)
    }

    /// The network listener the table names, if it names one.
    ///
    /// # Errors
    ///
    /// `listen` without a key it takes, in words naming the key.
    pub fn network(&self) -> Result<Option<Network<'_>>, String> {
        let Some(listen) = self.listen.as_deref() else {
            return Ok(None);
        };
        Ok(Some(Network {
            listen,
            certificate: given(listen, "certificate", self.certificate.as_deref())?,
            key: given(listen, "key", self.key.as_deref())?,
            anchors: given(listen, "anchors", self.anchors.as_deref())?,
        }))
    }
}

/// A key `listen` takes, or the refusal naming it.
fn given<'a>(listen: &str, key: &str, value: Option<&'a str>) -> Result<&'a str, String> {
    value.ok_or_else(|| format!("[operate] listen = \"{listen}\" needs {key}"))
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

    #[test]
    fn a_local_endpoint_stands_alone_and_listen_takes_its_keys() {
        let local: OperateConfiguration = toml::from_str("local = \"xmip-node\"\n").expect("read");
        assert_eq!(local.network(), Ok(None));
        // The local endpoint always starts: a table naming no listener is
        // a role for it.
        let neither: OperateConfiguration = toml::from_str("role = \"operator\"\n").expect("read");
        assert_eq!(neither.network(), Ok(None));
        assert!(neither.admit.is_empty(), "nobody else where it says none");
        let admitting: OperateConfiguration =
            toml::from_str("admit = ['CONTOSO\\xmip-operators', \"xmip\"]\n").expect("read");
        assert_eq!(admitting.admit, ["CONTOSO\\xmip-operators", "xmip"]);
        let short: OperateConfiguration =
            toml::from_str("listen = \"x:1\"\ncertificate = \"c\"\n").expect("read");
        assert_eq!(
            short.network(),
            Err("[operate] listen = \"x:1\" needs key".to_string())
        );
    }
}
