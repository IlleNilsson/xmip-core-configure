//! `[storage.database]`: the database server a Storage node is in front
//! of, where IT runs one (option A, `deployment-model.md` section 7).
//!
//! Xmip Storage keeps three databases on every backend, one to each data
//! domain — the runtime database, the administration database and the
//! audit database — and behind a server they are three separate databases,
//! which IT may place on different servers (the owner, 2026-10-01: *We
//! still need the distinction between runtime and administration
//! databases, regardless of backend database technology*; and 2026-10-10:
//! *The audit part might be better of in its own database so it can be
//! hosted on a different set of nodes, different storage*). So a Storage
//! node names three connections, one for each, and the secret its password
//! is kept under — a name, resolved through the key home, never the
//! password itself:
//!
//! ```toml
//! [storage.database]
//! runtime        = "postgresql://xmip_storage@db-1.example:5432/xmip_runtime"
//! administration = "postgresql://xmip_storage@db-2.example:5432/xmip_administration"
//! audit          = "postgresql://xmip_storage@db-3.example:5432/xmip_audit"
//! password       = "xmip-storage-database"
//! trust_anchor   = "/etc/xmip/database-authority.pem"
//! ```
//!
//! A connection is `<server>://<login>@<host>[:<port>]/<database>`; what it
//! may say, and the one reading of it, is Xmip Storage's
//! (`xmip-core-persist`, `storage::database`), which the runtime's
//! validation asks. Xmip always speaks TLS to the database server, its own
//! (ADR-0063); `trust_anchor` is the authority the server's certificate
//! reaches, relative to the configuration file, and the operating system's
//! trust store where it is left out. What a site's IT operators set up is
//! `deploy/database/<server>/README.md`.
//!
//! The form is the assistant's drafting of the owner's direction of
//! 2026-10-01 — *you just need to prepare what I, future IT-operators have
//! to do. A connection string, software to install or whatever* — and the
//! owner's to strike.

use serde::{Deserialize, Serialize};

/// `[storage.database]`, as the document says it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfiguration {
    /// The runtime database, the Ledger: a connection.
    pub runtime: String,
    /// The administration database: a connection.
    pub administration: String,
    /// The audit database: a connection.
    pub audit: String,
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
    fn three_connections_a_secret_and_an_anchor_are_read_and_nothing_else() {
        let audit = "audit = \"postgresql://xmip_storage@db-3/xmip_audit\"\n";
        let text = format!(
            "runtime = \"postgresql://xmip_storage@db-1/xmip_runtime\"\n\
             administration = \"postgresql://xmip_storage@db-2/xmip_administration\"\n\
             {audit}password = \"xmip-storage-database\"\n"
        );
        let database: DatabaseConfiguration = toml::from_str(&text).expect("reads");
        assert_eq!(database.audit, "postgresql://xmip_storage@db-3/xmip_audit");
        assert_eq!(database.password, "xmip-storage-database");
        let without = text.replace(audit, "");
        let missing = toml::from_str::<DatabaseConfiguration>(&without).expect_err("no audit");
        assert!(missing.to_string().contains("audit"), "{missing}");
        assert_eq!(database.trust_anchor, None);
        let written = toml::to_string(&database).expect("writes");
        assert_eq!(
            toml::from_str::<DatabaseConfiguration>(&written).expect("back"),
            database
        );
        let unknown = format!("{text}encrypt = false\n");
        let refused = toml::from_str::<DatabaseConfiguration>(&unknown).expect_err("unknown");
        assert!(refused.to_string().contains("encrypt"), "{refused}");
    }
}
