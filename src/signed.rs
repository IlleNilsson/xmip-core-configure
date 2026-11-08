//! A node's slice as its cluster signed it (ADR-0031, amendments
//! 2026-11-06 and 2026-11-07): the text a node runs, and under it the
//! `[signature]` table that says which save of `cluster.xmip` it is, the
//! node it is for, its serial and when it was signed, and who signed it.
//!
//! The form alone is here; the signing and the check are the runtime's,
//! with the cluster authority's key and certificate (`provision`). What is
//! signed is the text above the table, byte for byte, and every value of
//! the table but the signature itself ([`message`]), so an edit anywhere in
//! the file, the table included, no longer verifies.

use serde::{Deserialize, Serialize};

/// The line the signature's table opens with, at the start of a line.
pub const TABLE: &str = "[signature]";

/// What a slice's `[signature]` table holds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signature {
    /// Which save of the cluster's file the slice is: `sha256:` and the
    /// hexadecimal SHA-256 of that `cluster.xmip`.
    pub cluster: String,
    /// The node the slice is for, its `[service]` `node_name`: a node runs
    /// no other node's slice.
    pub node: String,
    /// Strictly increasing from one signing to the next: a node runs no
    /// slice older than the one it accepted.
    pub serial: u64,
    /// When it was signed, Unix seconds.
    pub issued: i64,
    /// Who signed it: `sha256:` and the hexadecimal SHA-256 of the signing
    /// certificate's DER.
    pub by: String,
    /// The signature over [`message`], base64.
    pub value: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    signature: Signature,
}

/// `text` taken apart at its signature: the text signed, and the signature
/// under it; `None` where it holds none.
///
/// # Errors
/// A `[signature]` table that is not its six values, in words.
pub fn split(text: &str) -> Result<(&str, Option<Signature>), String> {
    let at = if text.starts_with(TABLE) {
        Some(0)
    } else {
        text.find(&format!("\n{TABLE}")).map(|at| at + 1)
    };
    let Some(at) = at else {
        return Ok((text, None));
    };
    let table: Table = toml::from_str(&text[at..])
        .map_err(|error| format!("its {TABLE} table does not read: {}", error.message()))?;
    Ok((&text[..at], Some(table.signature)))
}

/// What the signature is over: the text signed, then the save, the node,
/// the serial and the time `signature` names, each on a line of its own;
/// its `by` and `value` are not, the certificate being what checks it.
#[must_use]
pub fn message(signed: &str, signature: &Signature) -> Vec<u8> {
    let said = format!(
        "\n{}\n{}\n{}\n{}",
        signature.cluster, signature.node, signature.serial, signature.issued
    );
    let mut message = Vec::with_capacity(signed.len() + said.len());
    message.extend_from_slice(signed.as_bytes());
    message.extend_from_slice(said.as_bytes());
    message
}

/// `signed` with `signature`'s table under it: the file a node is given.
#[must_use]
pub fn joined(signed: &str, signature: &Signature) -> String {
    let quote = |value: &str| toml::Value::String(value.to_string()).to_string();
    let mut text = String::from(signed);
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(TABLE);
    text.push_str(
        "\n# The cluster's authority signed this file as its cluster's file was\n\
         # saved; a node runs it only as it was signed (ADR-0031).\n",
    );
    text.push_str(&format!("cluster = {}\n", quote(&signature.cluster)));
    text.push_str(&format!("node = {}\n", quote(&signature.node)));
    text.push_str(&format!("serial = {}\n", signature.serial));
    text.push_str(&format!("issued = {}\n", signature.issued));
    text.push_str(&format!("by = {}\n", quote(&signature.by)));
    text.push_str(&format!("value = {}\n", quote(&signature.value)));
    text
}

/// The node a slice's text names under `[service]`, read alone so a slice
/// that is not yet a whole node still tells it; `None` where it names none
/// or the text does not read.
#[must_use]
pub fn named(text: &str) -> Option<String> {
    toml::from_str::<toml::Table>(text)
        .ok()?
        .remove("service")?
        .get("node_name")?
        .as_str()
        .map(str::to_string)
}
#[cfg(test)]
mod tests {
    use super::*;

    fn signature() -> Signature {
        Signature {
            cluster: "sha256:00".to_string(),
            node: "r1".to_string(),
            serial: 1_800_000_000_000_000_000,
            issued: 1_800_000_000,
            by: "sha256:11".to_string(),
            value: "AA==".to_string(),
        }
    }

    #[test]
    fn a_joined_slice_splits_into_the_text_signed_and_its_signature() {
        let signed = "[service]\nnode_name = \"r1\"\n";
        let file = joined(signed, &signature());
        assert_eq!(split(&file).expect("reads"), (signed, Some(signature())));
    }

    #[test]
    fn a_file_without_a_signature_is_all_text_and_a_table_with_more_refused() {
        assert_eq!(split("[service]\n").expect("reads"), ("[service]\n", None));
        let more = format!("{}extra = \"x\"\n", joined("", &signature()));
        assert!(split(&more).is_err());
        let fewer = joined("", &signature()).replace("serial = ", "# ");
        assert!(split(&fewer).is_err(), "a table without its serial");
    }

    #[test]
    fn the_message_covers_the_node_the_serial_and_the_time_but_not_the_value() {
        let signature = signature();
        let message = message("[service]\n", &signature);
        for other in [
            Signature {
                node: "r2".to_string(),
                ..signature.clone()
            },
            Signature {
                serial: signature.serial - 1,
                ..signature.clone()
            },
            Signature {
                issued: 0,
                ..signature.clone()
            },
        ] {
            assert_ne!(super::message("[service]\n", &other), message);
        }
        let revalued = Signature {
            value: "BB==".to_string(),
            ..signature.clone()
        };
        assert_eq!(super::message("[service]\n", &revalued), message);
    }
}
