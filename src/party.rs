//! `[[parties]]`: the Parties a cluster recognizes, each named with the
//! identities it is recognized by and Xmip presents when reaching it
//! (ADR-0019; `runtime-model.md`, ADR-0031 amendment 2026-10-01):
//!
//! ```toml
//! [[parties]]
//! name       = "<party>"
//! identities = ["CN=<party>.example"]
//! ```
//!
//! Written once at the top of the cluster's `xmip.toml`, so every node's
//! slice holds them ([`crate::cluster`]). Read here and nowhere else: the
//! node document's field ([`crate::XmipConfigurationDocument::parties`]) and
//! [`parties`], for a reader that holds a cluster's file and no node.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One Party: its name, which a Location names it by, and its identities.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartyConfiguration {
    pub name: String,
    /// The identities the Party is recognized by, each as its mechanism
    /// writes it — a certificate subject, a key's name.
    pub identities: Vec<String>,
}

/// What `[[parties]]` alone is read as, from any document holding it.
#[derive(Deserialize)]
struct Declared {
    #[serde(default)]
    parties: Vec<PartyConfiguration>,
}

/// The Parties `source` declares, in the order written: a cluster's file or
/// a node's configuration, read for `[[parties]]` alone.
///
/// # Errors
/// When the text is not TOML or a Party lacks its name or its identities.
pub fn parties(source: &str) -> Result<Vec<PartyConfiguration>, String> {
    toml::from_str::<Declared>(source)
        .map(|declared| declared.parties)
        .map_err(|error| error.to_string())
}

/// What is wrong with `parties`, one sentence each: a Party without a name,
/// a name used twice, a Party with no identity, and an identity two Parties
/// hold — it would resolve to neither.
#[must_use]
pub fn problems(parties: &[PartyConfiguration]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut named = BTreeMap::new();
    let mut held: BTreeMap<&str, &str> = BTreeMap::new();
    for party in parties {
        let name = party.name.as_str();
        if name.trim().is_empty() {
            problems.push("a Party requires a name".to_string());
        } else if named.insert(name, ()).is_some() {
            problems.push(format!("the Party '{name}' is declared twice"));
        }
        if party
            .identities
            .iter()
            .all(|identity| identity.trim().is_empty())
        {
            problems.push(format!("the Party '{name}' names no identity"));
        }
        for identity in &party.identities {
            if let Some(other) = held.insert(identity.as_str(), name)
                && other != name
            {
                problems.push(format!(
                    "the identity '{identity}' is held by the Parties '{other}' and '{name}'"
                ));
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::{PartyConfiguration, parties, problems};

    /// Two Parties named here and nowhere else, as a test's own data.
    fn two() -> String {
        let [first, second] = ["first", "second"].map(|word| format!("party-{word}"));
        format!(
            "[[parties]]\nname = \"{first}\"\nidentities = [\"CN={first}.example\"]\n\n\
             [[parties]]\nname = \"{second}\"\nidentities = [\"CN={second}.example\", \"k\"]\n"
        )
    }

    #[test]
    fn the_parties_read_in_order_with_their_identities() {
        let read = parties(&two()).expect("parses");
        assert_eq!(read.len(), 2);
        assert_eq!(read[0].identities, [format!("CN={}.example", read[0].name)]);
        assert_eq!(read[1].identities.len(), 2);
        assert!(problems(&read).is_empty(), "{:?}", problems(&read));
        assert_eq!(
            parties("[service]\nname = \"n\"\n").expect("none"),
            [] as [PartyConfiguration; 0]
        );
    }

    #[test]
    fn a_party_without_identities_or_with_an_unknown_key_is_refused() {
        let error = parties("[[parties]]\nname = \"p\"\n").expect_err("identities");
        assert!(error.contains("identities"), "{error}");
        let unknown = parties("[[parties]]\nname = \"p\"\nidentities = []\nrole = \"x\"\n");
        assert!(unknown.is_err());
    }

    #[test]
    fn a_name_twice_an_empty_party_and_a_shared_identity_are_problems() {
        let mut read = parties(&two()).expect("parses");
        let mut again = read[0].clone();
        again.identities.clone_from(&read[1].identities);
        read.push(again);
        read.push(super::PartyConfiguration {
            name: String::new(),
            identities: Vec::new(),
        });
        let said = problems(&read).join("\n");
        assert!(said.contains("declared twice"), "{said}");
        assert!(said.contains("is held by the Parties"), "{said}");
        assert!(said.contains("requires a name"), "{said}");
        assert!(said.contains("names no identity"), "{said}");
    }

    #[test]
    fn a_node_document_holds_the_parties_and_writes_them_back() {
        let text = format!(
            "[service]\nname = \"n\"\ncluster_name = \"c\"\nnode_name = \"d\"\n\n{}",
            two()
        );
        let document = crate::parse_toml(&text).expect("parses");
        assert_eq!(document.parties, parties(&text).expect("parses"));
        let written = toml::to_string(&document).expect("writes");
        assert_eq!(crate::parse_toml(&written).expect("reads back"), document);
    }
}
