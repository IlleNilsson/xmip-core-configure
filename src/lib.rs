use abi::{ExtensionManifest, ModuleManifest};
use serde::{Deserialize, Serialize};

// Arrived from the runtime's execution_tree on 2026-08-26. A configured
// service is what the runtime is built *from*, so it cannot live inside the
// thing it configures: runtime depended on configure and configure depended
// on runtime, and Cargo rejects that outright.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XmipServiceConfiguration {
    pub service_name: String,
    pub cluster_name: String,
    pub node_name: String,
    /// Whether this node may assume a route to the internet. False unless
    /// the operator says otherwise: nothing at runtime reaches out, and the
    /// switch records that it may, for the features and tests that need it
    /// (ADR-0045, 2026-09-10).
    #[serde(default)]
    pub online: bool,
    pub modules: Vec<ConfiguredModule>,
    pub xmip_processes: Vec<ConfiguredXmipProcess>,
    /// Where Xmip starts working — runtime-model.md. Added 2026-09-05 so a
    /// node has all three stages of the message path, not only Process.
    #[serde(default)]
    pub receive_locations: Vec<ConfiguredLocation>,
    /// Where a Message leaves.
    #[serde(default)]
    pub send_locations: Vec<ConfiguredLocation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfiguredModule {
    pub name: String,
    pub manifest: ModuleManifest,
    pub start: bool,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfiguredXmipProcess {
    pub name: String,
    pub start: bool,
    pub execution_style: ExecutionStyle,
    pub required_modules: Vec<String>,
    pub xmip_subprocesses: Vec<ConfiguredXmipSubprocess>,
    pub extensions: Vec<ExtensionManifest>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfiguredXmipSubprocess {
    pub name: String,
    pub required_modules: Vec<String>,
    pub extensions: Vec<ExtensionManifest>,
}

/// A Receive Location or a Send Location, as configured. One shape for both:
/// a name, the transport module that moves it, the address in that
/// transport's own terms, and whether it starts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfiguredLocation {
    pub name: String,
    pub start: bool,
    pub transport: String,
    pub address: String,
}

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
    #[serde(default)]
    pub receive_locations: Vec<ConfiguredLocation>,
    #[serde(default)]
    pub send_locations: Vec<ConfiguredLocation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceConfiguration {
    pub name: String,
    pub cluster_name: String,
    pub node_name: String,
    /// `online = true` when the node may assume the internet; omitted or
    /// false otherwise (ADR-0045).
    #[serde(default)]
    pub online: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleConfiguration {
    pub name: String,
    pub start: bool,
    pub manifest: ModuleManifest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XmipProcessConfiguration {
    pub name: String,
    pub start: bool,
    /// Defaults to `Sequential` when the document omits it, so an existing
    /// configuration reads unchanged.
    #[serde(default)]
    pub execution_style: ExecutionStyle,
    pub required_modules: Vec<String>,
    pub xmip_subprocesses: Vec<XmipSubprocessConfiguration>,
    pub extensions: Vec<ExtensionManifest>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XmipSubprocessConfiguration {
    pub name: String,
    pub required_modules: Vec<String>,
    pub extensions: Vec<ExtensionManifest>,
}

pub fn parse_toml(source: &str) -> Result<XmipConfigurationDocument, String> {
    toml::from_str(source).map_err(|error| error.to_string())
}

pub fn to_service_configuration(document: XmipConfigurationDocument) -> XmipServiceConfiguration {
    XmipServiceConfiguration {
        service_name: document.service.name,
        cluster_name: document.service.cluster_name,
        node_name: document.service.node_name,
        online: document.service.online,
        modules: document
            .modules
            .into_iter()
            .map(to_configured_module)
            .collect(),
        xmip_processes: document
            .xmip_processes
            .into_iter()
            .map(to_configured_process)
            .collect(),
        receive_locations: document.receive_locations,
        send_locations: document.send_locations,
    }
}

fn to_configured_module(module: ModuleConfiguration) -> ConfiguredModule {
    ConfiguredModule {
        name: module.name,
        manifest: module.manifest,
        start: module.start,
    }
}

fn to_configured_process(process: XmipProcessConfiguration) -> ConfiguredXmipProcess {
    ConfiguredXmipProcess {
        name: process.name,
        start: process.start,
        execution_style: process.execution_style,
        required_modules: process.required_modules,
        xmip_subprocesses: process
            .xmip_subprocesses
            .into_iter()
            .map(to_configured_subprocess)
            .collect(),
        extensions: process.extensions,
    }
}

fn to_configured_subprocess(subprocess: XmipSubprocessConfiguration) -> ConfiguredXmipSubprocess {
    ConfiguredXmipSubprocess {
        name: subprocess.name,
        required_modules: subprocess.required_modules,
        extensions: subprocess.extensions,
    }
}

pub fn parse_service_configuration(source: &str) -> Result<XmipServiceConfiguration, String> {
    parse_toml(source).map(to_service_configuration)
}

#[cfg(test)]
mod tests {
    use super::{ExecutionStyle, parse_toml, to_service_configuration};

    #[test]
    fn a_process_execution_style_parses_and_defaults_to_sequential() {
        let source = r#"
[service]
name = "n"
cluster_name = "c"
node_name = "d"

[[xmip_processes]]
name = "fast"
start = true
execution_style = "concurrent"
required_modules = []
xmip_subprocesses = []
extensions = []

[[xmip_processes]]
name = "ordered"
start = true
required_modules = []
xmip_subprocesses = []
extensions = []
"#;
        let document = parse_toml(source).expect("parses");
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
        let head = "[service]\nname = \"n\"\ncluster_name = \"c\"\nnode_name = \"d\"\n";
        let offline = parse_toml(head).expect("parses");
        assert!(!offline.service.online, "ADR-0045: offline unless said");
        assert!(!to_service_configuration(offline).online);
        let online = parse_toml(&format!("{head}online = true\n")).expect("parses");
        assert!(to_service_configuration(online).online);
    }
}
