//! A node's slice as its cluster signed it (ADR-0031, amendment
//! 2026-11-06): the text a node runs, and under it the `[signature]`
//! table that says which save of `cluster.xmip` it is and who signed it.
//!
//! The form alone is here; the signing and the check are the runtime's,
//! with the cluster authority's key and certificate (`provision`). What is
//! signed is the text above the table, byte for byte, and the save it names
//! ([`message`]), so an edit anywhere in the file, the table included, no
//! longer verifies.

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
/// A `[signature]` table that is not the three values, in words.
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

/// What the signature is over: the text signed, then the save it names.
#[must_use]
pub fn message(signed: &str, cluster: &str) -> Vec<u8> {
    let mut message = Vec::with_capacity(signed.len() + 1 + cluster.len());
    message.extend_from_slice(signed.as_bytes());
    message.push(b'\n');
    message.extend_from_slice(cluster.as_bytes());
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
    text.push_str(&format!("by = {}\n", quote(&signature.by)));
    text.push_str(&format!("value = {}\n", quote(&signature.value)));
    text
}

/// The node a slice's text names under `[service]`, and the data directory
/// it says, read alone so a slice that is not yet a whole node still tells
/// them; `None` for what it does not say or a text that does not read.
#[must_use]
pub fn named(text: &str) -> (Option<String>, Option<String>) {
    let service = toml::from_str::<toml::Table>(text)
        .ok()
        .and_then(|mut table| table.remove("service"));
    let said = |key: &str| {
        service
            .as_ref()
            .and_then(|service| service.get(key))
            .and_then(toml::Value::as_str)
            .map(str::to_string)
    };
    (said("node_name"), said("data"))
}
#[cfg(test)]
mod tests {
    use super::*;

    fn signature() -> Signature {
        Signature {
            cluster: "sha256:00".to_string(),
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
    }
}
