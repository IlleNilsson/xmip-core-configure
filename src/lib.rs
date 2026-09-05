use serde::{Deserialize, Serialize};
use xmip_abi::{ExtensionManifest, ModuleManifest};

// Arrived from the runtime's execution_tree on 2026-08-26. A configured
// service is what the runtime is built *from*, so it cannot live inside the
// thing it configures: runtime depended on configure and configure depended
// on runtime, and Cargo rejects that outright.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XmipServiceConfiguration {
    pub service_name: String,
    pub cluster_name: String,
    pub node_name: String,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfiguredXmipProcess {
    pub name: String,
    pub start: bool,
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
    pub modules: Vec<ModuleConfiguration>,
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
