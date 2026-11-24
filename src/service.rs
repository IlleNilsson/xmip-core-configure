//! `[service]`: the cluster and node a document configures, where its
//! data directory is, and where in it the node keeps its own files.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// `[service]`: the cluster and node this document configures.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceConfiguration {
    pub name: String,
    pub cluster_name: String,
    pub node_name: String,
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
    /// `configuration`: written in the platform's own separators, a `.`
    /// left out — `config\..\data\storage` never `config\../data\storage`
    /// — and, on Windows, a `..` taken back as the system itself resolves
    /// a path, by its words: `data\storage`. Elsewhere a `..` stays, since
    /// a directory before it may be a link.
    #[must_use]
    pub fn data_directory(&self, configuration: &Path) -> PathBuf {
        data_directory(self.data.as_deref(), configuration)
    }
}

/// The data directory `data` names, [`DEFAULT_DATA`] where it names none,
/// for the configuration file at `configuration`: as
/// [`ServiceConfiguration::data_directory`] says.
#[must_use]
pub fn data_directory(data: Option<&str>, configuration: &Path) -> PathBuf {
    let base = configuration.parent().unwrap_or_else(|| Path::new(""));
    let joined = base.join(data.unwrap_or(DEFAULT_DATA));
    let mut clean = PathBuf::new();
    for part in joined.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
                if cfg!(windows)
                    && matches!(
                        clean.components().next_back(),
                        Some(std::path::Component::Normal(_))
                    ) =>
            {
                clean.pop();
            }
            other => clean.push(other),
        }
    }
    clean
}

/// `<stem>.<kept>` in the default data directory of the configuration at
/// `path`, `../data` from its folder ([`data_directory`]): where the node
/// keeps a file of its own — its pin and its active configuration
/// (`crate::signed`), the number of its latest start (`crate::start`) —
/// found from the configuration's place alone, never from what a slice
/// says.
pub(crate) fn kept_at(path: &Path, kept: &str) -> PathBuf {
    let stem = path
        .file_stem()
        .map_or_else(|| "node".into(), |stem| stem.to_string_lossy());
    data_directory(None, path).join(format!("{stem}.{kept}"))
}
