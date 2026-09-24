//! The node configuration document, and the one reading of it.
//!
//! `XmipConfigurationDocument` is the one model of a node's configuration
//! in the estate. The runtime builds its execution tree from it, and the
//! desktop editor validates what it writes through the runtime's
//! `xmip_validate_v1` (ADR-0027, amendment 2026-09-05), which reads it here.
//! Until 2026-09-24 a second tree, `XmipServiceConfiguration`, restated every
//! field under other names and the runtime read that one; it is gone
//! (open problem 25, row b).
//!
//! What the runtime requires, the document requires: a location's `start`
//! and `transport` have no default, so a document without them is refused,
//! not completed.

use abi::{ExtensionManifest, ModuleManifest};
use serde::{Deserialize, Serialize};

/// One node's configuration, as the TOML on disk says it (ADR-0031).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XmipConfigurationDocument {
    pub service: ServiceConfiguration,
    // All four collections default to empty. A node with modules and no
    // processes is legitimate, and so is one an editor has only half built —
    // and a missing array should read as a validation problem an operator can
    // act on, not a "parse error at line 1" that points at nothing. Added
    // 2026-09-05, when the desktop editor needed to validate documents in
    // progress.
    #[serde(default)]
    pub modules: Vec<ModuleConfiguration>,
    #[serde(default)]
    pub xmip_processes: Vec<XmipProcessConfiguration>,
    /// Where Xmip starts working — runtime-model.md. Added 2026-09-05 so a
    /// node has all three stages of the message path, not only Process.
    #[serde(default)]
    pub receive_locations: Vec<ConfiguredLocation>,
    /// Where a Message leaves.
    #[serde(default)]
    pub send_locations: Vec<ConfiguredLocation>,
}

/// `[service]`: the cluster and node this document configures.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceConfiguration {
    pub name: String,
    pub cluster_name: String,
    pub node_name: String,
    /// Whether this node may assume a route to the internet. False unless
    /// the operator says otherwise: nothing at runtime reaches out, and the
    /// switch records that it may, for the features and tests that need it
    /// (ADR-0045, 2026-09-10).
    #[serde(default)]
    pub online: bool,
}

/// `[[modules]]`: a module the node loads, its manifest, and whether it starts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleConfiguration {
    pub name: String,
    pub start: bool,
    pub manifest: ModuleManifest,
}

/// How an Xmip Process runs its work, once claimed — runtime-model.md's
/// "execution style". `Sequential` is the safe default (one at a time, in order
/// per key); `Parallel` and `Concurrent` trade ordering for throughput, and are
/// the lever an operator raises when a node falls behind (the Playground's
/// `daily` scenario). Serialised kebab-case on the wire.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionStyle {
    /// One at a time, in order per key.
    #[default]
    Sequential,
    /// Many at once, no ordering guarantee.
    Parallel,
    /// Many in flight, interleaved, no ordering guarantee.
    Concurrent,
}

/// `[[xmip_processes]]`: one Xmip Process and what it needs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XmipProcessConfiguration {
    pub name: String,
    pub start: bool,
    /// Defaults to `Sequential` when the document omits it, so an existing
    /// configuration reads unchanged.
    #[serde(default)]
    pub execution_style: ExecutionStyle,
    /// The three lists below default to empty, as the document's own lists
    /// do: a Process that needs no module, has no Subprocess and no
    /// Extension says nothing about them (ADR-0031, amendment 2026-09-24).
    #[serde(default)]
    pub required_modules: Vec<String>,
    #[serde(default)]
    pub xmip_subprocesses: Vec<XmipSubprocessConfiguration>,
    #[serde(default)]
    pub extensions: Vec<ExtensionManifest>,
}

/// An Xmip Subprocess inside a Process.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XmipSubprocessConfiguration {
    pub name: String,
    /// Empty when omitted, as on the Process.
    #[serde(default)]
    pub required_modules: Vec<String>,
    #[serde(default)]
    pub extensions: Vec<ExtensionManifest>,
}

/// A Receive Location or a Send Location, as configured. One shape for both:
/// a name, the transport module that moves it, the address in that
/// transport's own terms, and whether it starts. None of the four has a
/// default.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfiguredLocation {
    pub name: String,
    pub start: bool,
    pub transport: String,
    pub address: String,
}

/// Read a node configuration document. The error is the TOML reader's own
/// words, line and column included.
///
/// # Errors
/// When the text is not TOML, or lacks a key the document requires.
pub fn parse_toml(source: &str) -> Result<XmipConfigurationDocument, String> {
    toml::from_str(source).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{ExecutionStyle, parse_toml};

    const HEAD: &str = "[service]\nname = \"n\"\ncluster_name = \"c\"\nnode_name = \"d\"\n";

    #[test]
    fn a_process_execution_style_parses_and_defaults_to_sequential() {
        let source = format!(
            "{HEAD}
[[xmip_processes]]
name = \"fast\"
start = true
execution_style = \"concurrent\"
required_modules = []
xmip_subprocesses = []
extensions = []

[[xmip_processes]]
name = \"ordered\"
start = true
required_modules = []
xmip_subprocesses = []
extensions = []
"
        );
        let document = parse_toml(&source).expect("parses");
        assert_eq!(
            document.xmip_processes[0].execution_style,
            ExecutionStyle::Concurrent
        );
        assert_eq!(
            document.xmip_processes[1].execution_style,
            ExecutionStyle::Sequential,
            "an omitted style defaults to Sequential"
        );
    }

    #[test]
    fn a_node_is_offline_unless_the_document_says_online() {
        let offline = parse_toml(HEAD).expect("parses");
        assert!(!offline.service.online, "ADR-0045: offline unless said");
        let online = parse_toml(&format!("{HEAD}online = true\n")).expect("parses");
        assert!(online.service.online);
    }

    #[test]
    fn a_process_that_names_no_list_reads_them_as_empty() {
        let source = format!("{HEAD}[[xmip_processes]]\nname = \"minimal\"\nstart = true\n");
        let document = parse_toml(&source).expect("parses");
        let process = &document.xmip_processes[0];
        assert!(process.required_modules.is_empty());
        assert!(process.xmip_subprocesses.is_empty());
        assert!(process.extensions.is_empty());
    }

    #[test]
    fn a_location_without_start_is_refused_not_defaulted() {
        let source = format!(
            "{HEAD}[[receive_locations]]\nname = \"in\"\ntransport = \"file\"\n\
             address = \"C:/in\"\n"
        );
        let error = parse_toml(&source).expect_err("start is required");
        assert!(error.contains("start"), "names the missing key: {error}");
    }

    #[test]
    fn a_location_without_transport_is_refused_not_defaulted() {
        let source = format!(
            "{HEAD}[[send_locations]]\nname = \"out\"\nstart = true\naddress = \"C:/out\"\n"
        );
        let error = parse_toml(&source).expect_err("transport is required");
        assert!(
            error.contains("transport"),
            "names the missing key: {error}"
        );
    }

    #[test]
    fn an_escaped_string_reads_as_what_it_says_and_round_trips() {
        let source = format!(
            "{HEAD}[[receive_locations]]\nname = \"say \\\"hi\\\"\"\nstart = true\n\
             transport = \"file\"\naddress = \"C:\\\\in\\\\t\\u00e9st\\nnext\"\n"
        );
        let document = parse_toml(&source).expect("parses");
        let location = &document.receive_locations[0];
        assert_eq!(location.name, "say \"hi\"");
        assert_eq!(location.address, "C:\\in\\t\u{e9}st\nnext");

        let written = toml::to_string(&document).expect("writes");
        assert_eq!(parse_toml(&written).expect("reads back"), document);
    }
}
