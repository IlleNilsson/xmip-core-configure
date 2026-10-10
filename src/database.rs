//! `[runtime]`, `[administration]` and `[audit]`: what each of Xmip
//! Storage's three data domains is kept on, and `[storage.database]`: the
//! secret a database server's password is kept under (option A,
//! `deployment-model.md` section 7).
//!
//! Each domain is a table of its own, its `storage` and its `connection`
//! (the owner, 2026-10-10: *i would do it like runtime, storage, connection
//! string. Same for audit and administration*, and *Yes, better*), and each
//! may be on another technology and another server:
//!
//! ```toml
//! [runtime]
//! storage    = "postgresql"
//! connection = "host=db-1.example port=5432 dbname=xmip_runtime user=xmip_storage"
//!
//! [administration]
//! storage    = "postgresql"
//! connection = "host=db-2.example port=5432 dbname=xmip_administration user=xmip_storage"
//!
//! [audit]
//! storage    = "sqlite"
//! connection = "D:/Xmip/data/storage/audit.sqlite"
//!
//! [storage.database]
//! password     = "xmip-storage-database"
//! trust_anchor = "/etc/xmip/database-authority.pem"
//! ```
//!
//! `storage` is `rocksdb`, `sqlite`, `postgresql` or `mssql`; on an
//! embedded engine `connection` is the store's path, relative to the
//! configuration file, and on a database server the server's own
//! connection string. What each may say, and the one reading of it, is
//! Xmip Storage's (`xmip-core-persist`, `storage::database`), which the
//! runtime's validation asks. A table left out is the embedded Storage
//! node's own, under the node's data directory (`xmip-core-runtime`,
//! `storage.rs`).
//!
//! The password is never in a connection: `password` names the secret it
//! is kept under — a name, resolved through the key home, never the
//! password itself. Xmip always speaks TLS to the database server, its own
//! (ADR-0063); `trust_anchor` is the authority the server's certificate
//! reaches, relative to the configuration file, and the operating system's
//! trust store where it is left out. What a site's IT operators set up is
//! `deploy/database/<server>/README.md`.

use serde::{Deserialize, Serialize};

/// `[runtime]`, `[administration]` or `[audit]`, as the document says it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainConfiguration {
    /// What the domain's database is kept on, by its word.
    pub storage: String,
    /// Its connection string, or its path on an embedded engine.
    pub connection: String,
}

/// `[storage.database]`, as the document says it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfiguration {
    /// The name of the secret the login's password is kept under.
    pub password: String,
    /// The authority the server's certificate reaches, PEM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_anchor: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_domain_is_its_storage_and_its_connection_and_nothing_else() {
        let text = "storage = \"postgresql\"\nconnection = \"host=db-1 dbname=xmip_runtime\"\n";
        let domain: DomainConfiguration = toml::from_str(text).expect("reads");
        assert_eq!(domain.storage, "postgresql");
        let written = toml::to_string(&domain).expect("writes");
        assert_eq!(
            toml::from_str::<DomainConfiguration>(&written).expect("back"),
            domain
        );
        let missing = toml::from_str::<DomainConfiguration>("storage = \"sqlite\"\n");
        assert!(
            missing
                .expect_err("no connection")
                .to_string()
                .contains("connection")
        );
        let unknown = format!("{text}encrypt = false\n");
        let refused = toml::from_str::<DomainConfiguration>(&unknown).expect_err("unknown");
        assert!(refused.to_string().contains("encrypt"), "{refused}");
    }

    #[test]
    fn a_secret_and_an_anchor_are_read_and_no_connection() {
        let text = "password = \"xmip-storage-database\"\n";
        let database: DatabaseConfiguration = toml::from_str(text).expect("reads");
        assert_eq!(database.password, "xmip-storage-database");
        assert_eq!(database.trust_anchor, None);
        let old = format!("{text}runtime = \"host=h dbname=d\"\n");
        let refused = toml::from_str::<DatabaseConfiguration>(&old).expect_err("unknown");
        assert!(refused.to_string().contains("runtime"), "{refused}");
    }
}
