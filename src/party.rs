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
//!
//! A Receive Location names the Parties it takes in `accept.party`
//! (ADR-0019 clause 1), and a verified identity is resolved to the Party
//! holding it ([`holding`]): each Party is known to the gates by an
//! identifier ([`id`]), and a name `accept.party` gives that no Party here
//! holds is a problem every node's binding reports ([`accepted_problems`]).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use xcore::PartyId;

use crate::ConfiguredLocation;

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

/// The identifier the gates know the Party at `place` among `[[parties]]`
/// by, counted from one: what a verified identity resolves to, what
/// `xmip.party` holds for a filter and what the audit records. Every node
/// of a cluster holds the same `[[parties]]` in the same order, so each
/// gives a Party the same identifier. The assistant's draft of 2026-10-19,
/// after the Playground's own counting, for the owner to overrule: a Party
/// written in another place has another identifier.
#[must_use]
pub fn id(place: usize) -> PartyId {
    PartyId::new(place as u128 + 1)
}

/// The place among `parties` of the Party holding the identity `value`, as
/// its mechanism wrote it, where one does.
#[must_use]
pub fn place_holding(parties: &[PartyConfiguration], value: &str) -> Option<usize> {
    parties
        .iter()
        .position(|party| party.identities.iter().any(|held| held == value))
}

/// The Party holding the identity `value`, by its identifier: what a
/// verified identity resolves to.
#[must_use]
pub fn holding(parties: &[PartyConfiguration], value: &str) -> Option<PartyId> {
    place_holding(parties, value).map(id)
}

/// The Party named `name`, by its identifier.
#[must_use]
pub fn named(parties: &[PartyConfiguration], name: &str) -> Option<PartyId> {
    parties.iter().position(|party| party.name == name).map(id)
}

/// Every name `location`'s `accept.party` gives that no Party among
/// `parties` is called, one sentence each.
#[must_use]
pub fn accepted_problems(
    parties: &[PartyConfiguration],
    location: &ConfiguredLocation,
) -> Vec<String> {
    location
        .accept
        .party
        .iter()
        .filter(|name| named(parties, name).is_none())
        .map(|name| {
            format!(
                "the Receive Location '{}' accepts the Party '{name}', which [[parties]] \
                 does not declare",
                location.name
            )
        })
        .collect()
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
    fn an_identity_resolves_to_the_party_holding_it_counted_from_one() {
        let read = parties(&two()).expect("parses");
        let second = &read[1];

        assert_eq!(super::holding(&read, "k"), Some(super::id(1)));
        assert_eq!(super::holding(&read, "CN=nobody.example"), None);
        assert_eq!(super::named(&read, &second.name), Some(super::id(1)));
        assert_eq!(
            super::named(&read, &read[0].name),
            Some(xcore::PartyId::new(1))
        );
        assert_eq!(super::named(&read, "absent"), None);
    }

    #[test]
    fn a_receive_location_accepting_a_party_none_declares_is_a_problem() {
        let read = parties(&two()).expect("parses");
        let text = format!(
            "[service]\nname = \"n\"\ncluster_name = \"c\"\nnode_name = \"d\"\n\n{}\n\
             [[receive_locations]]\nname = \"in\"\ntransport = \"t\"\naddress = \"a\"\n\
             [receive_locations.accept]\nmechanism = [\"mutual-tls\"]\n\
             party = [\"{}\", \"absent\"]\n",
            two(),
            read[0].name
        );
        let document = crate::parse_toml(&text).expect("parses");
        let location = &document.receive_locations[0];

        assert_eq!(location.accept.party.len(), 2);
        assert_eq!(
            super::accepted_problems(&document.parties, location),
            [
                "the Receive Location 'in' accepts the Party 'absent', which [[parties]] does \
              not declare"
            ]
        );
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
