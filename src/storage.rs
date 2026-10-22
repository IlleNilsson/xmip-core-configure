//! `[storage]`: the Storage nodes a node reaches Xmip Storage at.
//!
//! Every node calls Xmip Storage — the nodes declaring the Storage role —
//! for all storage, never a database directly, and finds them from a list
//! of their addresses in its configuration, tried round robin (the owner,
//! 2026-10-01, asked whether a node should find them from a list of their
//! addresses in its TOML, tried round robin: *Yes*; `deployment-model.md`
//! section 7):
//!
//! ```toml
//! [storage]
//! nodes = ["storage-1.example:7443", "storage-2.example:7443"]
//! listen = "0.0.0.0:7443"             # where a Storage node serves; the node's address
//! certificate = "tls/node.pem"        # this node's chain, leaf first
//! key = "tls/node.key"                # its private key
//! anchors = "tls/anchors.pem"         # the cluster's own authority
//! ```
//!
//! Each address is `host:port`, a host name or an address, IPv6 in
//! brackets; the list names each once. More than one is the safety: a node
//! carries on through the next while one is stopped. Every Storage node of
//! the cluster lists the same nodes, itself among them, in the same order:
//! the first listed is the first primary, and the others its standbys
//! (ADR-0056, amendment 2026-10-22).
//!
//! The table is shaped as `[operate]` is (ADR-0067, amendment 2026-10-13):
//! `listen` is where a node declaring the Storage role serves Xmip Storage,
//! read by such a node alone, and `certificate`, `key` and `anchors`, all
//! three or none, are what the node presents and trusts over Xmip's mutual
//! TLS — to the Storage nodes it reaches, and as one to the nodes reaching
//! it — paths relative to the configuration file.
//!
//! A Storage node in front of a database server IT runs names that server
//! in `[storage.database]` ([`crate::database`]).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::database::DatabaseConfiguration;

/// `[storage]`, as the document says it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageConfiguration {
    /// The Storage nodes, in the order round robin starts from.
    #[serde(default)]
    pub nodes: Vec<StorageAddress>,
    /// Where a node declaring the Storage role serves Xmip Storage:
    /// `host:port`, the node's address and the Storage plane's port
    /// (ADR-0067, amendment 2026-10-13). Read by such a node alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen: Option<String>,
    /// This node's PEM certificate chain, leaf first: what it presents to
    /// the Storage nodes it reaches and, serving, to the nodes reaching it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate: Option<String>,
    /// Its PEM private key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// The PEM anchors the other end's certificate must reach: the
    /// cluster's own authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchors: Option<String>,
    /// The database server this node is in front of, where it is a
    /// Storage node before one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database: Option<DatabaseConfiguration>,
}

/// What a node presents and trusts over Xmip's TLS to and as a Storage
/// node, all three given; paths relative to the configuration file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Presented<'a> {
    pub certificate: &'a str,
    pub key: &'a str,
    pub anchors: &'a str,
}

impl StorageConfiguration {
    /// Whether the document says nothing of its Storage nodes.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }

    /// What the node presents and trusts, where the table names it.
    ///
    /// # Errors
    ///
    /// One of `certificate`, `key` and `anchors` named without the others,
    /// in words naming what is missing.
    pub fn presented(&self) -> Result<Option<Presented<'_>>, String> {
        let files = [
            ("certificate", self.certificate.as_deref()),
            ("key", self.key.as_deref()),
            ("anchors", self.anchors.as_deref()),
        ];
        match files.map(|(_, file)| file) {
            [None, None, None] => Ok(None),
            [Some(certificate), Some(key), Some(anchors)] => Ok(Some(Presented {
                certificate,
                key,
                anchors,
            })),
            _ => {
                let missing: Vec<&str> = files
                    .iter()
                    .filter_map(|(name, file)| file.is_none().then_some(*name))
                    .collect();
                Err(format!(
                    "[storage] names certificate, key and anchors together, and misses {}",
                    missing.join(" and ")
                ))
            }
        }
    }

    /// What is wrong with the table as a whole, in words: a Storage node
    /// named twice, a certificate without its key or anchors. What a
    /// connection to a database server may say is Xmip Storage's to judge,
    /// and the runtime's validation asks it.
    #[must_use]
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for (index, node) in self.nodes.iter().enumerate() {
            if self.nodes[..index].contains(node) {
                problems.push(format!("[storage] nodes names '{node}' twice"));
            }
        }
        if let Err(problem) = self.presented() {
            problems.push(problem);
        }
        problems
    }
}

/// One Storage node's address, `host:port`, read once here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct StorageAddress(String);

impl StorageAddress {
    /// The address as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for StorageAddress {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        let refused = |why: &str| format!("[storage] nodes: '{text}' {why}; write host:port");
        let Some((host, port)) = text.rsplit_once(':') else {
            return Err(refused("names no port"));
        };
        let host = host
            .strip_prefix('[')
            .and_then(|h| h.strip_suffix(']'))
            .unwrap_or(host);
        if host.is_empty() || host.contains(char::is_whitespace) {
            return Err(refused("names no host"));
        }
        if host.contains(':') && !text.starts_with('[') {
            return Err(refused("is an IPv6 address out of brackets"));
        }
        match port.parse::<u16>() {
            Ok(port) if port > 0 => Ok(Self(text)),
            _ => Err(refused("names no port from 1 to 65535")),
        }
    }
}

impl From<StorageAddress> for String {
    fn from(address: StorageAddress) -> Self {
        address.0
    }
}

impl fmt::Display for StorageAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Result<StorageConfiguration, String> {
        toml::from_str(text).map_err(|error| error.to_string())
    }

    #[test]
    fn the_storage_nodes_are_read_in_order_as_written() {
        let storage =
            read("nodes = [\"storage-1.example:7443\", \"10.0.0.7:7443\", \"[fd00::7]:7443\"]\n")
                .expect("reads");
        let nodes: Vec<&str> = storage.nodes.iter().map(StorageAddress::as_str).collect();
        assert_eq!(
            nodes,
            ["storage-1.example:7443", "10.0.0.7:7443", "[fd00::7]:7443"]
        );
        assert_eq!(storage.problems(), [] as [String; 0]);
        assert!(read("").expect("reads").is_default());
    }

    #[test]
    fn an_address_without_a_host_or_a_port_is_refused_in_words() {
        for (text, why) in [
            ("storage-1.example", "names no port"),
            (":7443", "names no host"),
            ("storage-1.example:0", "from 1 to 65535"),
            ("storage-1.example:http", "from 1 to 65535"),
            ("fd00::7:7443", "out of brackets"),
        ] {
            let refused = read(&format!("nodes = [\"{text}\"]\n")).expect_err(text);
            assert!(refused.contains(why), "{text}: {refused}");
        }
        assert!(
            read("color = \"lime\"\n")
                .expect_err("unknown")
                .contains("color")
        );
    }

    #[test]
    fn what_a_node_presents_is_named_whole_or_not_at_all() {
        let named = read(
            "listen = \"0.0.0.0:7443\"\ncertificate = \"tls/node.pem\"\n\
             key = \"tls/node.key\"\nanchors = \"tls/anchors.pem\"\n",
        )
        .expect("reads");
        assert_eq!(
            named.presented(),
            Ok(Some(Presented {
                certificate: "tls/node.pem",
                key: "tls/node.key",
                anchors: "tls/anchors.pem",
            }))
        );
        assert_eq!(named.listen.as_deref(), Some("0.0.0.0:7443"));
        let half = read("certificate = \"tls/node.pem\"\n").expect("reads");
        assert_eq!(
            half.problems(),
            ["[storage] names certificate, key and anchors together, and misses key and anchors"]
        );
        assert_eq!(read("").expect("reads").presented(), Ok(None));
    }

    #[test]
    fn a_storage_node_named_twice_is_a_problem() {
        let twice = read("nodes = [\"s1:7443\", \"s2:7443\", \"s1:7443\"]\n").expect("reads");
        assert_eq!(twice.problems(), ["[storage] nodes names 's1:7443' twice"]);
    }
}
