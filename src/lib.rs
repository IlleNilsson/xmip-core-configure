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
//!
//! The second document is the Xmip Application (ADR-0064): an integration as
//! a developer designs it — its Receive Locations, Subscriptions, Xmip
//! Processes and Send Ports, with no environment in it — read by
//! [`parse_application`] and checked by [`application_problems`]. A node's
//! configuration *binds* Applications (`[[applications]]`, [`binding`]) and
//! [`bind`] joins the two into what one node runs. The designer's view of an
//! Application — its [`routes`], a filter's structure ([`filter`]) and the
//! edits it makes ([`edit`]) — is here too, because what the design means is
//! this crate's to say; the language server reaches it through the runtime's
//! library.

pub mod application;
pub mod binding;
pub mod edit;
pub mod entry;
pub mod filter;
pub mod routes;
pub mod settings;
pub mod store;

pub use application::{
    ApplicationHeader, DesignedElement, SendPortGroup, XmipApplicationDocument,
    application_problems, parse_application,
};
pub use binding::{ApplicationBinding, Bound, BoundLocation, bind, binding_problems};
pub use entry::subscription_entry;
pub use settings::{Declarations, LocationSettings, location_problems};
pub use store::StoreConfiguration;

use std::path::{Path, PathBuf};

use abi::{ExtensionManifest, ModuleManifest};
use serde::{Deserialize, Serialize};

/// The words every problem opens with that is the TOML reader's, for either
/// document: a surface shows it whole, because its message spans lines.
pub const PARSE_FAILED: &str = "configuration parse failed";

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
    /// The Xmip Applications this node runs, each with the environment's
    /// side of what it declares (ADR-0064). Empty when the node binds none.
    #[serde(default)]
    pub applications: Vec<ApplicationBinding>,
    /// Where the node keeps its runtime store and the key store sealing it
    /// ([`store`], ADR-0018 amendment 2026-09-30). Absent, the installed
    /// layout's.
    #[serde(default, skip_serializing_if = "StoreConfiguration::is_default")]
    pub store: StoreConfiguration,
}

/// Which of the two documents a text is. An Xmip Application opens with its
/// `[application]` table; everything else is read as a node's configuration,
/// whose reading says what is wrong with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentKind {
    /// A node's configuration, [`XmipConfigurationDocument`].
    Node,
    /// An Xmip Application, [`XmipApplicationDocument`].
    Application,
}

/// Which document `source` is, by the table it holds: never by its file's
/// name.
#[must_use]
pub fn document_kind(source: &str) -> DocumentKind {
    match source.parse::<toml::Table>() {
        Ok(table) if table.contains_key("application") => DocumentKind::Application,
        _ => DocumentKind::Node,
    }
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
    /// The node's data directory: where its runtime store, its keys and the
    /// orders an operator leaves for it are kept unless `[store]` says
    /// otherwise. Relative to the configuration file; absent, `../data`,
    /// which in the installed layout (ADR-0015 clause 10) is the `data`
    /// beside the `config` the file is in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

/// Where a node's data directory is when its configuration does not say.
pub const DEFAULT_DATA: &str = "../data";

impl ServiceConfiguration {
    /// The node's data directory, for the configuration file at
    /// `configuration`.
    #[must_use]
    pub fn data_directory(&self, configuration: &Path) -> PathBuf {
        let base = configuration.parent().unwrap_or_else(|| Path::new(""));
        base.join(self.data.as_deref().unwrap_or(DEFAULT_DATA))
    }
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
/// default. `credentials` names the secret the Location presents or checks —
/// a reference, never the secret — and is absent where it needs none.
/// `settings` is what the transport takes beyond the address, `contract` the
/// contract module a Stream is held to and `contract_settings` what that
/// takes: each table read through its technology's own declaration
/// ([`settings::location_problems`], ADR-0064 amendment 2026-09-26), and
/// empty or absent where the Location gives none. `accept` is what a
/// Receive Location takes ([`Accept`]); a Send Location presents and gives
/// none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfiguredLocation {
    pub name: String,
    pub start: bool,
    pub transport: String,
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<String>,
    #[serde(default, skip_serializing_if = "LocationSettings::is_empty")]
    pub settings: LocationSettings,
    #[serde(default, skip_serializing_if = "LocationSettings::is_empty")]
    pub contract_settings: LocationSettings,
    #[serde(default, skip_serializing_if = "Accept::is_empty")]
    pub accept: Accept,
}

/// `accept`: the closed set of mechanisms a Receive Location authenticates
/// (ADR-0019 clause 1), each by the name its mechanism declares —
/// `circumstance`, `mutual-tls`, `oauth2`. An identity presented by any
/// other is refused at authentication and never tried against the rest.
/// Absent or empty, the Location takes nothing: an unconfigured endpoint is
/// closed, not open.
///
/// ```toml
/// [receive_locations.accept]
/// mechanism = ["mutual-tls", "oauth2"]
/// ```
///
/// ADR-0019 writes a `party` list beside `mechanism`, narrowing the set to
/// named Parties; a node's configuration names no Party yet, so the key is
/// refused rather than read as nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Accept {
    #[serde(default)]
    pub mechanism: Vec<String>,
}

impl Accept {
    /// Whether the Location declares no mechanism: it takes nothing.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.mechanism.is_empty()
    }
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
    fn the_data_directory_is_the_layouts_unless_the_document_names_one() {
        let path = std::path::Path::new("/opt/xmip/config/xmip-node.toml");
        let installed = parse_toml(HEAD).expect("parses");
        assert_eq!(
            installed.service.data_directory(path),
            std::path::Path::new("/opt/xmip/config/../data")
        );
        assert!(installed.store.is_default());
        let named = parse_toml(&format!("{HEAD}data = \"state\"\n[store]\nkeys = \"k\"\n"))
            .expect("parses");
        assert_eq!(
            named.service.data_directory(path),
            std::path::Path::new("/opt/xmip/config/state")
        );
        assert_eq!(named.store.keys.as_deref(), Some("k"));
        assert_eq!(
            parse_toml(&toml::to_string(&named).expect("writes")).expect("reads back"),
            named
        );
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
