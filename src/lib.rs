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
//! What the runtime requires, the document requires: a Location's
//! `address` has no default, so a document without it is refused, not
//! completed. The scheme it is entered with chooses its transport
//! ([`ConfiguredLocation::scheme`]). A Location starts unless its `start`
//! says `false`, when it is known and not started ([`NOT_STARTED`]).
//!
//! An Xmip Application (ADR-0064) — an integration as a developer designs
//! it: its Receive Ports and Locations, Subscriptions, Work Processes and
//! Send Ports, with no environment in it — is a section of the cluster's
//! one `xmip.toml`, `[[xmip_applications]]` ([`section`]), read by
//! [`parse_application`]. A node's configuration *binds* the sections it
//! runs (`[[applications]]`, [`binding`]) and [`bind`] joins the two into
//! what one node runs; where an environment writes no binding by hand,
//! [`placed`] places each Location on its nodes. The designer's view of an Application — its
//! [`routes`], a filter's structure ([`filter`]) and the edits it makes
//! ([`edit`]) — is here too, and so is the designer's view of the cluster's
//! one `xmip.toml`, artifact by artifact ([`views`]), and the edits it makes
//! there ([`view_edit`]), because what the design means is this crate's to
//! say; the language server reaches it through the runtime's library. So
//! is what an editor suggests, looks up and renames by (ADR-0064, amendment
//! 2026-10-13): the keys each table may hold, read from these types
//! ([`keys`], [`shape`]), where each table and key is written ([`written`]),
//! and what names what ([`references`]).

pub mod application;
pub mod binding;
pub mod cluster;
pub mod correlation;
pub mod database;
pub mod edit;
mod element;
pub mod entry;
pub mod field;
pub mod filter;
#[cfg(any(test, feature = "test-support"))]
pub mod fixture;
pub mod keys;
mod kinds;
pub mod modules;
pub mod offered;
pub mod operate;
pub mod party;
pub mod placed;
pub mod port;
mod probe;
mod reference_edit;
pub mod references;
pub mod routes;
pub mod section;
pub mod service;
pub mod settings;
pub mod shape;
pub mod standard;
pub mod step;
pub mod storage;
pub mod store;
pub mod view_edit;
pub mod views;
pub mod work_process;
pub mod written;

pub use application::{SendPortGroup, XmipApplication, parse_application};
pub use binding::{
    ApplicationBinding, Bound, BoundLocation, BoundReceivePort, bind, binding_problems,
};
pub use cluster::{is_cluster, slice, slice_named, slices};
pub use correlation::Correlation;
pub use database::DomainConfiguration;
pub use entry::subscription_entry;
pub use modules::ModuleConfiguration;
pub use operate::OperateConfiguration;
pub use party::{PartyConfiguration, PartyIdentity};
pub use port::{
    Depth, DesignedReceiveLocation, DesignedReceivePort, DesignedSendPort, DesignedWorkProcess,
    Failover, Interaction, OnFailure, Retry,
};
pub use section::{ApplicationSection, applications};
pub use service::{DEFAULT_DATA, ServiceConfiguration};
pub use settings::{Declarations, LocationSettings, location_problems};
pub use step::{Attached, StepEntry};
pub use storage::{Presented, StorageConfiguration};
pub use store::StoreConfiguration;
pub use work_process::WorkProcessConfiguration;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The words every problem opens with that is the TOML reader's: a surface
/// shows it whole, because its message spans lines.
pub const PARSE_FAILED: &str = "configuration parse failed";

/// What every surface shows for a Receive Location, Send Port or Send
/// Location whose `start` says `false`: it is known and not started (the
/// owner, 2026-10-13; ADR-0064, amendment 2026-10-28).
pub const NOT_STARTED: &str = "not started: the configuration says false";

/// A Location's `start` where it says none: a Location entered by its
/// address starts, a bound Send Port as a Location (ADR-0064, amendment
/// 2026-10-28).
const fn starts() -> bool {
    true
}

/// One node's configuration, as the TOML on disk says it (ADR-0031).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XmipConfigurationDocument {
    /// The roles the node declares, as its cluster's `[nodes.<name>]`
    /// [`cluster::ROLES`] writes them and the slice gives them: role words
    /// by comma or plus, whose parse is `node::NodeRole`'s. Absent, it
    /// declares none. Only a node declaring `storage` keeps Xmip Storage
    /// itself (ADR-0056, amendment 2026-10-20).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roles: Option<String>,
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
    /// The calls to compiled units the node's own Locations attach by name
    /// ([`step`], ADR-0066 amendment 2026-10-22).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preparation_steps: Vec<StepEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub promotions: Vec<StepEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub demotions: Vec<StepEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transformations: Vec<StepEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub debatches: Vec<StepEntry>,
    /// What work is spread over, declared once for the cluster
    /// (`correlation`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub correlations: Vec<Correlation>,
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
    /// `[[parties]]`: the Parties the cluster recognizes, each with its
    /// identities ([`party`], ADR-0019). Empty when it names none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parties: Vec<PartyConfiguration>,
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
    /// `[provision.<purpose>]`: where the node's certificates come from, and
    /// on the node whose authority signs a person's request,
    /// `[provision.operator]` (ADR-0034, amendment 2026-10-15). Each table
    /// as written, read through `xmip-core-provision`'s reading of it; empty
    /// where the document names none.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub provision: BTreeMap<String, LocationSettings>,
    /// `[operate]`: the node's operate listener, where a surface on this
    /// machine, at the local endpoint, or on another, at the network
    /// address, reads, follows and acts on it ([`operate`], ADR-0067).
    /// Absent, the node serves none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operate: Option<OperateConfiguration>,
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

/// A Receive Location or a Send Location, as configured. One shape for both:
/// a name, the address it is entered by — `sftp://party-x@host/in`,
/// `https://party-x.example/orders`, `C:\drop` — and whether it starts. The
/// two have no default; the address's scheme chooses the transport that
/// moves it ([`ConfiguredLocation::scheme`]), and it starts unless `start`
/// says `false`, when it is known and not started, [`NOT_STARTED`] (ADR-0064,
/// amendment 2026-10-28). `credentials` names the secret the Location presents or checks —
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
    #[serde(default = "starts")]
    pub start: bool,
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
    /// Its Preparation Steps in order, its Transformation and, on receive,
    /// its Promotion and Debatch or, on send, its Demotion: each by its
    /// entry's name ([`step`], ADR-0066 amendments 2026-10-22 and
    /// 2026-10-27).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prepare: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promote: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub demote: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debatch: Option<String>,
    /// A Send Location's Correlation ([`correlation`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation: Option<String>,
}

/// `accept`: the closed set a Receive Location authenticates (ADR-0019
/// clause 1) — the mechanisms, each by the name its mechanism declares,
/// `circumstance`, `mutual-tls`, `oauth2`, and, where it narrows to them,
/// the Parties, each by its name in `[[parties]]` ([`party`]). An identity
/// presented by any other mechanism is refused at authentication and never
/// tried against the rest; with `party` given, a verified identity that
/// resolves to another Party, or to none, is refused there too. Absent or
/// empty, the Location takes nothing: an unconfigured endpoint is closed,
/// not open.
///
/// ```toml
/// [receive_locations.accept]
/// mechanism = ["mutual-tls", "oauth2"]
/// party     = ["<party>"]
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Accept {
    #[serde(default)]
    pub mechanism: Vec<String>,
    /// The Parties the Location takes, by name; empty, any identity the
    /// mechanisms verify.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub party: Vec<String>,
}

impl Accept {
    /// Whether the Location declares nothing it takes: no mechanism and no
    /// Party.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.mechanism.is_empty() && self.party.is_empty()
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
    use super::parse_toml;
    use abi::ExtensionManifest;

    const HEAD: &str = "[service]\nname = \"n\"\ncluster_name = \"c\"\nnode_name = \"d\"\n";

    #[test]
    fn each_provision_table_is_kept_as_written_by_its_purpose() {
        let text = format!(
            "{HEAD}
[provision.operator]
authority = \"authority\"
lifetime = \"8h\"
"
        );
        let document = parse_toml(&text).expect("parses");
        let operator = &document.provision["operator"];
        assert_eq!(operator.0["lifetime"].as_str(), Some("8h"));
        assert!(parse_toml(HEAD).expect("parses").provision.is_empty());
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
        let layout = if cfg!(windows) {
            "/opt/xmip/data"
        } else {
            "/opt/xmip/config/../data"
        };
        assert_eq!(
            installed.service.data_directory(path),
            std::path::Path::new(layout)
        );
        // In the platform's own separators, whatever the document wrote.
        let shown = installed
            .service
            .data_directory(std::path::Path::new("config/./xmip-node.toml"))
            .join("storage")
            .display()
            .to_string();
        let other = if cfg!(windows) { '/' } else { '\\' };
        assert!(!shown.contains(other), "{shown}");
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
        assert_eq!(process.required_modules, [] as [String; 0]);
        assert_eq!(process.extensions, [] as [ExtensionManifest; 0]);
    }

    #[test]
    fn a_location_entered_by_its_address_starts_unless_it_says_false() {
        let source = format!(
            "{HEAD}[[receive_locations]]\nname = \"in\"\naddress = \"C:/in\"\n\
             [[send_locations]]\nname = \"out\"\naddress = \"sftp://h/out\"\nstart = false\n"
        );
        let document = parse_toml(&source).expect("an address is enough");
        assert!(document.receive_locations[0].start, "nothing means started");
        assert!(!document.send_locations[0].start);
    }

    #[test]
    fn a_location_without_an_address_is_refused_not_defaulted() {
        let source = format!("{HEAD}[[send_locations]]\nname = \"out\"\n");
        let error = parse_toml(&source).expect_err("an address is required");
        assert!(error.contains("address"), "names the missing key: {error}");
    }

    #[test]
    fn an_escaped_string_reads_as_what_it_says_and_round_trips() {
        let source = format!(
            "{HEAD}[[receive_locations]]\nname = \"say \\\"hi\\\"\"\nstart = true\n\
             address = \"C:\\\\in\\\\t\\u00e9st\\nnext\"\n"
        );
        let document = parse_toml(&source).expect("parses");
        let location = &document.receive_locations[0];
        assert_eq!(location.name, "say \"hi\"");
        assert_eq!(location.address, "C:\\in\\t\u{e9}st\nnext");

        let written = toml::to_string(&document).expect("writes");
        assert_eq!(parse_toml(&written).expect("reads back"), document);
    }
}
