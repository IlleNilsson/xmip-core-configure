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
//! ```
//!
//! Each address is `host:port`, a host name or an address, IPv6 in
//! brackets; the list names each once. More than one is the safety: a node
//! carries on through the next while one is stopped.
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
    /// The database server this node is in front of, where it is a
    /// Storage node before one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database: Option<DatabaseConfiguration>,
}

impl StorageConfiguration {
    /// Whether the document says nothing of its Storage nodes.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }

    /// What is wrong with the list as a whole, in words: a Storage node
    /// named twice. What a connection to a database server may say is Xmip
    /// Storage's to judge, and the runtime's validation asks it.
    #[must_use]
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for (index, node) in self.nodes.iter().enumerate() {
            if self.nodes[..index].contains(node) {
                problems.push(format!("[storage] nodes names '{node}' twice"));
            }
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
        assert!(storage.problems().is_empty());
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
            read("colour = \"lime\"\n")
                .expect_err("unknown")
                .contains("colour")
        );
    }

    #[test]
    fn a_storage_node_named_twice_is_a_problem() {
        let twice = read("nodes = [\"s1:7443\", \"s2:7443\", \"s1:7443\"]\n").expect("reads");
        assert_eq!(twice.problems(), ["[storage] nodes names 's1:7443' twice"]);
    }
}
