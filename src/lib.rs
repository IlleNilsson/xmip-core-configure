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
//! An Xmip Application (ADR-0064) — an integration as a developer designs
//! it: its Receive Ports and Locations, Subscriptions, Work Processes and
//! Send Ports, with no environment in it — is a section of the cluster's
//! one `xmip.toml`, `[[xmip_applications]]` ([`section`]), read by
//! [`parse_application`]. A node's configuration *binds* the sections it
//! runs (`[[applications]]`, [`binding`]) and [`bind`] joins the two into
//! what one node runs. The designer's view of an Application — its
//! [`routes`], a filter's structure ([`filter`]) and the edits it makes
//! ([`edit`]) — is here too, and so is the designer's view of the cluster's
//! one `xmip.toml`, artifact by artifact ([`views`]), and the edits it makes
//! there ([`view_edit`]), because what the design means is this crate's to
//! say; the language server reaches it through the runtime's library.

pub mod application;
pub mod binding;
pub mod cluster;
pub mod database;
pub mod edit;
pub mod entry;
pub mod field;
pub mod filter;
#[cfg(any(test, feature = "test-support"))]
pub mod fixture;
pub mod port;
pub mod routes;
pub mod section;
pub mod settings;
pub mod storage;
pub mod store;
pub mod view_edit;
pub mod views;

pub use application::{DesignedElement, SendPortGroup, XmipApplication, parse_application};
pub use binding::{
    ApplicationBinding, Bound, BoundLocation, BoundReceivePort, bind, binding_problems,
};
pub use cluster::{is_cluster, slice, slices};
pub use database::DomainConfiguration;
pub use entry::subscription_entry;
pub use port::{
    Depth, DesignedReceiveLocation, DesignedSendPort, Failover, Interaction, OnFailure, Retry,
};
pub use section::ApplicationSection;
pub use settings::{Declarations, LocationSettings, location_problems};
pub use storage::StorageConfiguration;
pub use store::StoreConfiguration;

use std::path::{Path, PathBuf};

use abi::{ExtensionManifest, ModuleManifest};
use serde::{Deserialize, Serialize};

/// The words every problem opens with that is the TOML reader's: a surface
/// shows it whole, because its message spans lines.
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
    pub work_processes: Vec<WorkProcessConfiguration>,
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
    /// The Xmip Applications held as sections of the cluster's `xmip.toml`,
    /// `[[xmip_applications]]` ([`section`], ADR-0064 amendment
    /// 2026-10-03): those this node's bindings name, as the slice gives
    /// them. A binding binds the section of its name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub xmip_applications: Vec<ApplicationSection>,
    /// What each of Xmip Storage's three data domains is kept on, each
    /// its storage and its connection ([`database`], the owner,
    /// 2026-10-10). Absent, the embedded Storage node's own, under the
    /// node's data directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<DomainConfiguration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub administration: Option<DomainConfiguration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit: Option<DomainConfiguration>,
    /// The key store an embedded Storage node's records are sealed under
    /// ([`store`], ADR-0018 amendments 2026-09-30 and 2026-10-03). Absent,
    /// the platform's, keeping its keys in the installed layout's place.
    #[serde(default, skip_serializing_if = "StoreConfiguration::is_default")]
    pub store: StoreConfiguration,
    /// The Storage nodes this node reaches Xmip Storage at, round robin
    /// ([`storage`], `deployment-model.md` section 7), and the database
    /// server a Storage node is in front of.
    #[serde(default, skip_serializing_if = "StorageConfiguration::is_default")]
    pub storage: StorageConfiguration,
    /// `[tuning]`: every outward and hardware assumption the node runs by —
    /// the TCP segment, the segments a chunk holds, the receive pool, the
    /// Storage client's timeout and pass-over (ADR-0031, amendment
    /// 2026-10-03). Which keys there are, their kinds, bounds and defaults
    /// are the runtime's declaration, which reads the table as a
    /// technology reads a Location's settings; empty, every default.
    #[serde(default, skip_serializing_if = "LocationSettings::is_empty")]
    pub tuning: LocationSettings,
}

impl XmipConfigurationDocument {
    /// The three data domains' tables, by the word each is named by, in
    /// the order runtime, administration, audit; `None` where one is left
    /// out.
    #[must_use]
    pub fn domains(&self) -> [(&'static str, Option<&DomainConfiguration>); 3] {
        [
            ("runtime", self.runtime.as_ref()),
            ("administration", self.administration.as_ref()),
            ("audit", self.audit.as_ref()),
        ]
    }
}

/// Which of the two documents a text is. A cluster's `xmip.toml` declares
/// its nodes in `[nodes]` ([`cluster`]); everything else is read as a
/// node's configuration, whose reading says what is wrong with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentKind {
    /// A node's configuration, [`XmipConfigurationDocument`].
    Node,
    /// A cluster's `xmip.toml`, each node's document sliced from it
    /// ([`slice`]).
    Cluster,
}

/// Which document `source` is, by the table it holds: never by its file's
/// name.
#[must_use]
pub fn document_kind(source: &str) -> DocumentKind {
    if is_cluster(source) {
        DocumentKind::Cluster
    } else {
        DocumentKind::Node
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
    /// The node's data directory: where its embedded Storage node, its keys
    /// and the orders an operator leaves for it are kept, the keys unless
    /// `[store]` says otherwise. Relative to the configuration file; absent, `../data`,
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

/// How a Work Process runs its work, once claimed — runtime-model.md's
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

/// `[[work_processes]]`: one Work Process and what it needs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkProcessConfiguration {
    pub name: String,
    pub start: bool,
    /// Defaults to `Sequential` when the document omits it, so an existing
    /// configuration reads unchanged.
    #[serde(default)]
    pub execution_style: ExecutionStyle,
    /// The two lists below default to empty, as the document's own lists
    /// do: a Work Process that needs no module and no Extension says
    /// nothing about them (ADR-0031, amendment 2026-09-24).
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
/// When the text is not TOML, lacks a key the document requires, or is a
/// cluster's `xmip.toml`, which a node reads only as its slice.
pub fn parse_toml(source: &str) -> Result<XmipConfigurationDocument, String> {
    let document = toml::from_str(source).map_err(|error: toml::de::Error| error.to_string())?;
    if is_cluster(source) {
        return Err(
            "this is a cluster's xmip.toml, which declares [nodes]; a node reads the slice \
             deployment writes it (configure::slice)"
                .to_string(),
        );
    }
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::{ExecutionStyle, parse_toml};

    const HEAD: &str = "[service]\nname = \"n\"\ncluster_name = \"c\"\nnode_name = \"d\"\n";

    #[test]
    fn a_process_execution_style_parses_and_defaults_to_sequential() {
        let source = format!(
            "{HEAD}
[[work_processes]]
name = \"fast\"
start = true
execution_style = \"concurrent\"
required_modules = []
extensions = []

[[work_processes]]
name = \"ordered\"
start = true
required_modules = []
extensions = []
"
        );
        let document = parse_toml(&source).expect("parses");
        assert_eq!(
            document.work_processes[0].execution_style,
            ExecutionStyle::Concurrent
        );
        assert_eq!(
            document.work_processes[1].execution_style,
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
    fn each_data_domain_is_a_table_of_its_own_or_left_out() {
        let text = format!(
            "{HEAD}[runtime]\nstorage = \"postgresql\"\n\
             connection = \"host=db-1 port=5432 dbname=xmip_runtime\"\n\
             [audit]\nstorage = \"sqlite\"\nconnection = \"D:/Xmip/data/storage/audit.sqlite\"\n"
        );
        let document = parse_toml(&text).expect("parses");
        let domains = document.domains();
        let words: Vec<&str> = domains.iter().map(|(word, _)| *word).collect();
        assert_eq!(words, ["runtime", "administration", "audit"]);
        assert_eq!(domains[0].1.map(|d| d.storage.as_str()), Some("postgresql"));
        assert_eq!(domains[1].1, None, "left out: the embedded node's own");
        assert_eq!(domains[2].1.map(|d| d.storage.as_str()), Some("sqlite"));
        assert_eq!(
            parse_toml(&toml::to_string(&document).expect("writes")).expect("reads back"),
            document
        );
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
    fn a_clusters_file_is_told_apart_and_not_read_as_a_node() {
        let node = crate::fixture::test_cluster().node(0).name.clone();
        let cluster = format!("{HEAD}[nodes.{node}]\n");
        assert_eq!(super::document_kind(&cluster), super::DocumentKind::Cluster);
        assert!(parse_toml(&cluster).expect_err("refused").contains("slice"));
    }

    #[test]
    fn a_process_that_names_no_list_reads_them_as_empty() {
        let source = format!("{HEAD}[[work_processes]]\nname = \"minimal\"\nstart = true\n");
        let document = parse_toml(&source).expect("parses");
        let process = &document.work_processes[0];
        assert!(process.required_modules.is_empty());
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
