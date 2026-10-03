//! `[store]`: the key store an embedded Storage node's records are sealed
//! under (ADR-0018, amendments 2026-09-30 and 2026-10-03).
//!
//! A node with no Storage node listed in `[storage]` is its own embedded
//! Storage node (`deployment-model.md` section 3), and Xmip encrypts its
//! databases itself: every record sealed under a data key a key store wraps
//! (ADR-0063). The engines are no node's choice: the runtime database is
//! always `RocksDB` (ADR-0015 and ADR-0018, amendments 2026-10-01). A
//! node's configuration names the key store by its module name, as a
//! Location names its transport, and where a key store that keeps files
//! keeps them. Every key may be left out:
//!
//! ```toml
//! [store]
//! key_store = "xmip-core-secret-file"     # the platform's, by default
//! keys = "/opt/xmip/data/key"
//! ```
//!
//! A relative path is relative to the configuration file, as an Xmip
//! Application's `document` is. The default is under the node's data
//! directory, `[service] data` ([`crate::ServiceConfiguration::data`]): the
//! platform's key store (ADR-0063 clause 4) — DPAPI on Windows, the keychain
//! on macOS, a private file on every other Unix — keeping its keys at
//! `<data>/key`.
//!
//! Which key stores there are is never this crate's: the program that
//! starts a node links them, and a node naming one its program was not
//! built with is refused as it starts.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The runtime database's one engine on an embedded Storage node, by module
/// name: `RocksDB`, always (ADR-0015, amendment 2026-10-01).
pub const ENGINE: &str = "xmip-core-persist-rocksdb";

/// Where a key store that keeps files keeps them beneath the data directory.
pub const DEFAULT_KEYS: &str = "key";

/// The platform's key store (ADR-0063 clause 4), where the configuration
/// names none.
pub const PLATFORM_KEY_STORE: &str = if cfg!(windows) {
    "xmip-core-secret-dpapi"
} else if cfg!(target_os = "macos") {
    "xmip-core-secret-keychain"
} else {
    "xmip-core-secret-file"
};

/// `[store]`, as the document says it: each key absent where it is left
/// out.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreConfiguration {
    /// The key store the data key is wrapped by, by module name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_store: Option<String>,
    /// Where a key store that keeps files keeps its keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<String>,
}

/// The key store `[store]` names, every default taken and every path
/// resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Store {
    pub key_store: String,
    pub keys: PathBuf,
}

impl StoreConfiguration {
    /// Whether the document says nothing of its key store.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }

    /// The key store this says, with `data` the node's data directory and
    /// `base` the directory the configuration file is in.
    #[must_use]
    pub fn resolve(&self, data: &Path, base: &Path) -> Store {
        Store {
            key_store: self
                .key_store
                .clone()
                .unwrap_or_else(|| PLATFORM_KEY_STORE.to_string()),
            keys: self
                .keys
                .as_ref()
                .map_or_else(|| data.join(DEFAULT_KEYS), |keys| base.join(keys)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Result<StoreConfiguration, String> {
        toml::from_str(text).map_err(|error| error.to_string())
    }

    #[test]
    fn an_unnamed_key_store_is_the_platforms_keeping_its_keys_under_the_data_directory() {
        let store = StoreConfiguration::default()
            .resolve(Path::new("/opt/xmip/data"), Path::new("/opt/xmip/config"));
        assert_eq!(store.key_store, PLATFORM_KEY_STORE);
        assert_eq!(store.keys, Path::new("/opt/xmip/data/key"));
    }

    #[test]
    fn a_named_key_store_is_read_relative_to_the_configuration() {
        let store = read("key_store = \"xmip-core-secret-file\"\nkeys = \"key\"\n")
            .expect("reads")
            .resolve(Path::new("/opt/xmip/data"), Path::new("/etc/xmip"));
        assert_eq!(store.key_store, "xmip-core-secret-file");
        assert_eq!(store.keys, Path::new("/etc/xmip/key"));
    }

    #[test]
    fn an_engine_or_a_place_is_no_choice_and_an_unknown_key_is_refused() {
        let engine = read("engine = \"xmip-core-persist-sqlite\"\n").expect_err("refused");
        assert!(engine.contains("engine"), "{engine}");
        let place = read("place = \"state/runtime\"\n").expect_err("refused");
        assert!(place.contains("place"), "{place}");
        let unknown = read("colour = \"lime\"\n").expect_err("refused");
        assert!(unknown.contains("colour"), "{unknown}");
    }
}
