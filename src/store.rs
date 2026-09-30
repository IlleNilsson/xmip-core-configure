//! `[store]`: where a node keeps its runtime store, and the key store its
//! records are sealed under (ADR-0018, amendment 2026-09-30).
//!
//! The runtime store is persist's `EncryptedStore` over one engine, every
//! record sealed under a data key a key store wraps (ADR-0063, ADR-0015
//! amendment 2026-09-25). A node's configuration names the engine and the
//! key store by their module names, as a Location names its transport, and
//! where each keeps its bytes. Every key may be left out, and a node whose
//! configuration has no `[store]` at all keeps its store where the installed
//! layout puts it:
//!
//! ```toml
//! [store]
//! engine = "xmip-core-persist-rocksdb"    # the default
//! place = "/opt/xmip/data/persistence-rocksdb"
//! key_store = "xmip-core-secret-file"     # the platform's, by default
//! keys = "/opt/xmip/data/key"
//! ```
//!
//! A relative path is relative to the configuration file, as an Xmip
//! Application's `document` is. The defaults are under the node's data
//! directory, `[service] data` ([`crate::ServiceConfiguration::data`]):
//! the engine `RocksDB`, the runtime store's (ADR-0015, amendment
//! 2026-09-25), at `<data>/persistence-rocksdb` (`deployment-model.md`
//! section 6); the platform's key store (ADR-0063 clause 4) — DPAPI on
//! Windows, the keychain on macOS, a private file on every other Unix —
//! keeping its keys at `<data>/key`. A store that names an engine other
//! than the default names its place too: a default place is the default
//! engine's.
//!
//! Which engines and key stores there are is never this crate's: the
//! program that starts a node links them, and a node naming one its program
//! was not built with is refused as it starts.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The runtime store's engine where the configuration names none.
pub const DEFAULT_ENGINE: &str = "xmip-core-persist-rocksdb";

/// Where the default engine keeps its store beneath the data directory.
pub const DEFAULT_PLACE: &str = "persistence-rocksdb";

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
    /// The engine, by module name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    /// Where the engine keeps its bytes: a directory for `RocksDB`, a file
    /// for `SQLite`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    /// The key store the data key is wrapped by, by module name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_store: Option<String>,
    /// Where a key store that keeps files keeps its keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<String>,
}

/// A node's runtime store, every default taken and every path resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Store {
    pub engine: String,
    pub place: PathBuf,
    pub key_store: String,
    pub keys: PathBuf,
}

impl StoreConfiguration {
    /// Whether the document says nothing of its store.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }

    /// The store this says, with `data` the node's data directory and
    /// `base` the directory the configuration file is in.
    ///
    /// # Errors
    /// An engine named without its place, in words.
    pub fn resolve(&self, data: &Path, base: &Path) -> Result<Store, String> {
        let place = match (&self.engine, &self.place) {
            (_, Some(place)) => base.join(place),
            (Some(engine), None) if engine != DEFAULT_ENGINE => {
                return Err(format!(
                    "[store] names the engine '{engine}' and no place; a default place is \
                     {DEFAULT_ENGINE}'s"
                ));
            }
            (_, None) => data.join(DEFAULT_PLACE),
        };
        Ok(Store {
            engine: self
                .engine
                .clone()
                .unwrap_or_else(|| DEFAULT_ENGINE.to_string()),
            place,
            key_store: self
                .key_store
                .clone()
                .unwrap_or_else(|| PLATFORM_KEY_STORE.to_string()),
            keys: self
                .keys
                .as_ref()
                .map_or_else(|| data.join(DEFAULT_KEYS), |keys| base.join(keys)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Result<StoreConfiguration, String> {
        toml::from_str(text).map_err(|error| error.to_string())
    }

    #[test]
    fn an_unnamed_store_is_rocksdb_under_the_data_directory_sealed_by_the_platform() {
        let store = StoreConfiguration::default()
            .resolve(Path::new("/opt/xmip/data"), Path::new("/opt/xmip/config"))
            .expect("resolved");
        assert_eq!(store.engine, "xmip-core-persist-rocksdb");
        assert_eq!(store.place, Path::new("/opt/xmip/data/persistence-rocksdb"));
        assert_eq!(store.key_store, PLATFORM_KEY_STORE);
        assert_eq!(store.keys, Path::new("/opt/xmip/data/key"));
    }

    #[test]
    fn a_named_store_is_read_relative_to_the_configuration() {
        let store = read(
            "engine = \"xmip-core-persist-sqlite\"\nplace = \"state/runtime.sqlite\"\n\
             key_store = \"xmip-core-secret-file\"\nkeys = \"/var/lib/xmip/key\"\n",
        )
        .expect("reads")
        .resolve(Path::new("/opt/xmip/data"), Path::new("/etc/xmip"))
        .expect("resolved");
        assert_eq!(store.engine, "xmip-core-persist-sqlite");
        assert_eq!(store.place, Path::new("/etc/xmip/state/runtime.sqlite"));
        assert_eq!(store.key_store, "xmip-core-secret-file");
        assert_eq!(store.keys, Path::new("/var/lib/xmip/key"));
    }

    #[test]
    fn another_engine_names_its_place_and_an_unknown_key_is_refused() {
        let refused = read("engine = \"xmip-core-persist-sqlite\"\n")
            .expect("reads")
            .resolve(Path::new("data"), Path::new(""))
            .expect_err("no place");
        assert!(refused.contains("xmip-core-persist-sqlite"), "{refused}");
        let unknown = read("colour = \"lime\"\n").expect_err("refused");
        assert!(unknown.contains("colour"), "{unknown}");
    }
}
