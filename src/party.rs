//! `[[parties]]`: the Parties a cluster recognizes, each with its
//! identifier and the identities it is recognized by and Xmip presents when
//! reaching it (ADR-0019; `runtime-model.md`, ADR-0031 amendment
//! 2026-10-01):
//!
//! ```toml
//! [[parties]]
//! id         = "0199f3a2-5c4e-7b1d-9a3f-2e8c6d4b1a07"
//! name       = "<party>"
//! identities = [
//!   { mechanism = "mutual-tls", purpose = "receive", value = "CN=<party>.example" },
//! ]
//! ```
//!
//! Written once at the top of the cluster's `xmip.toml`, so every node's
//! slice holds them ([`crate::cluster`]). Read here and nowhere else: the
//! node document's field ([`crate::XmipConfigurationDocument::parties`]) and
//! [`parties`], for a reader that holds a cluster's file and no node.
//!
//! **A Party's identifier is written with it** (the owner, 2026-10-19): a
//! UUIDv7, minted once when the Party is written ([`PartyConfiguration::new`])
//! and never derived from where it stands in the list. Every surface that
//! writes a Party writes one, and a Party without one is a problem every
//! node's binding reports, in words ([`problems`]).
//!
//! **Each identity states its mechanism and its purpose** (the owner,
//! 2026-10-19; ADR-0019 clause 4: *per direction and per mechanism*): the
//! mechanism that verifies it, by the name the mechanism declares, and
//! whether it is for receiving — matched against what arrives — or for
//! sending — what Xmip presents. A verified identity resolves to the Party
//! holding that value under that mechanism for that purpose ([`holding`]),
//! and to none otherwise.
//!
//! A Receive Location names the Parties it takes in `accept.party`
//! (ADR-0019 clause 1), and a name `accept.party` gives that no Party here
//! holds is a problem every node's binding reports ([`accepted_problems`]).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use xcore::{IdGenerator, PartyId, Purpose, UuidV7Generator};

use crate::ConfiguredLocation;

/// One Party: its identifier, its name, which a Location names it by, and
/// its identities.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartyConfiguration {
    pub name: String,
    /// The identities the Party is recognized by or presented as.
    pub identities: Vec<PartyIdentity>,
    /// The identifier the gates know it by, a UUIDv7 written once with the
    /// Party: what a verified identity resolves to, what `xmip.party`
    /// holds for a filter and what the audit records. Absent, the Party is
    /// refused in words ([`problems`]). Declared last: the designer's
    /// reading of the keys (`crate::shape`) stops a table at a value its
    /// reader refuses, and a UUID refuses what the reading offers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<PartyId>,
}

impl PartyConfiguration {
    /// A Party written now: `name`, its `identities`, and an identifier
    /// minted for it, once. What every surface that writes a Party calls.
    #[must_use]
    pub fn new(name: impl Into<String>, identities: Vec<PartyIdentity>) -> Self {
        Self {
            id: Some(mint()),
            name: name.into(),
            identities,
        }
    }
}

/// The list `[[parties]]` is, by its key.
pub const PARTIES: &str = "parties";

/// The key a Party's identifier is written under.
pub const ID: &str = "id";

/// A Party's identifier, minted now: a UUIDv7, written once with the Party
/// and never again.
#[must_use]
pub fn mint() -> PartyId {
    PartyId::new(UuidV7Generator.next_u128())
}

/// One identity a Party holds: the mechanism that verifies it, what it is
/// for, and its value as that mechanism writes it — a certificate subject,
/// a key's name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartyIdentity {
    /// The mechanism's name, as it declares it: `mutual-tls`, `api-key`.
    pub mechanism: String,
    /// `receive` or `send` ([`PURPOSES`]).
    #[serde(with = "purpose")]
    pub purpose: Purpose,
    pub value: String,
}

impl PartyIdentity {
    /// The identity `value` under `mechanism`, for `purpose`.
    #[must_use]
    pub fn new(mechanism: impl Into<String>, purpose: Purpose, value: impl Into<String>) -> Self {
        Self {
            mechanism: mechanism.into(),
            purpose,
            value: value.into(),
        }
    }
}

/// What a Party's identity may be for: receiving, matched against what
/// arrives, and sending, what Xmip presents (the owner, 2026-10-19).
pub const PURPOSES: [Purpose; 2] = [Purpose::Receive, Purpose::Send];

/// An identity's `purpose`, read and written in `xcore::Purpose`'s words,
/// and read as a choice of [`PURPOSES`], so the designer offers those two.
mod purpose {
    use std::fmt;

    use serde::de::{EnumAccess, Error, VariantAccess, Visitor};
    use serde::{Deserializer, Serializer};
    use xcore::Purpose;

    use super::PURPOSES;

    /// The words [`PURPOSES`] are written in.
    const WORDS: [&str; 2] = [PURPOSES[0].word(), PURPOSES[1].word()];

    pub(super) fn serialize<S: Serializer>(purpose: &Purpose, out: S) -> Result<S::Ok, S::Error> {
        out.serialize_str(purpose.word())
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(read: D) -> Result<Purpose, D::Error> {
        read.deserialize_enum("purpose", &WORDS, Words)
    }

    /// The purpose `word` names, or why none does.
    fn named<E: Error>(word: &str) -> Result<Purpose, E> {
        PURPOSES
            .into_iter()
            .find(|purpose| purpose.word() == word)
            .ok_or_else(|| {
                E::custom(format!(
                    "a Party's identity is for '{}' or '{}', not '{word}'",
                    WORDS[0], WORDS[1]
                ))
            })
    }

    struct Words;

    impl<'de> Visitor<'de> for Words {
        type Value = Purpose;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "'{}' or '{}'", WORDS[0], WORDS[1])
        }

        fn visit_str<E: Error>(self, word: &str) -> Result<Purpose, E> {
            named(word)
        }

        fn visit_enum<A: EnumAccess<'de>>(self, data: A) -> Result<Purpose, A::Error> {
            let (word, variant): (String, _) = data.variant()?;
            variant.unit_variant()?;
            named(&word)
        }
    }
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
/// When the text is not TOML, a Party lacks its name or its identities, an
/// identity lacks its mechanism, purpose or value, or an id is not a UUID.
pub fn parties(source: &str) -> Result<Vec<PartyConfiguration>, String> {
    toml::from_str::<Declared>(source)
        .map(|declared| declared.parties)
        .map_err(|error| error.to_string())
}

/// The place among `parties` of the Party holding `value` under
/// `mechanism` for `purpose`, where one does.
#[must_use]
pub fn place_holding(
    parties: &[PartyConfiguration],
    mechanism: &str,
    purpose: Purpose,
    value: &str,
) -> Option<usize> {
    parties.iter().position(|party| {
        party.identities.iter().any(|held| {
            held.mechanism == mechanism && held.purpose == purpose && held.value == value
        })
    })
}

/// The Party holding `value` under `mechanism` for `purpose`, by its
/// identifier: what a verified identity resolves to.
#[must_use]
pub fn holding(
    parties: &[PartyConfiguration],
    mechanism: &str,
    purpose: Purpose,
    value: &str,
) -> Option<PartyId> {
    place_holding(parties, mechanism, purpose, value).and_then(|place| parties[place].id)
}

/// The Party named `name`, by its identifier.
#[must_use]
pub fn named(parties: &[PartyConfiguration], name: &str) -> Option<PartyId> {
    parties
        .iter()
        .find(|party| party.name == name)
        .and_then(|party| party.id)
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
        .filter(|name| !parties.iter().any(|party| &party.name == *name))
        .map(|name| {
            format!(
                "the Receive Location '{}' accepts the Party '{name}', which [[parties]] \
                 does not declare",
                location.name
            )
        })
        .collect()
}

/// Whether `id` is a UUIDv7 (RFC 9562): version 7, the RFC's variant.
#[must_use]
pub const fn is_uuid_v7(id: PartyId) -> bool {
    let value = id.value();
    (value >> 76) & 0xf == 7 && (value >> 62) & 0b11 == 0b10
}

/// What is wrong with `parties`, one sentence each: a Party without a name,
/// a name used twice, a Party without an id or with one that is not a
/// UUIDv7 or another Party's, a Party with no identity, an identity naming
/// no mechanism or no value, and an identity two Parties hold — it would
/// resolve to neither.
#[must_use]
pub fn problems(parties: &[PartyConfiguration]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut named = BTreeMap::new();
    let mut ids = BTreeMap::new();
    let mut held: BTreeMap<(&str, Purpose, &str), &str> = BTreeMap::new();
    for party in parties {
        let name = party.name.as_str();
        if name.trim().is_empty() {
            problems.push("a Party requires a name".to_string());
        } else if named.insert(name, ()).is_some() {
            problems.push(format!("the Party '{name}' is declared twice"));
        }
        match party.id {
            None => problems.push(format!(
                "the Party '{name}' has no id: a Party is written with its id, a UUIDv7 \
                 minted once with it"
            )),
            Some(id) if !is_uuid_v7(id) => problems.push(format!(
                "the Party '{name}' has the id '{id}', which is not a UUIDv7"
            )),
            Some(id) => {
                if let Some(other) = ids.insert(id, name) {
                    problems.push(format!(
                        "the Parties '{other}' and '{name}' have one id, '{id}'"
                    ));
                }
            }
        }
        if party.identities.is_empty() {
            problems.push(format!("the Party '{name}' names no identity"));
        }
        for identity in &party.identities {
            let (mechanism, value) = (identity.mechanism.trim(), identity.value.trim());
            if mechanism.is_empty() || value.is_empty() {
                problems.push(format!(
                    "the Party '{name}' has an identity naming no mechanism or no value"
                ));
                continue;
            }
            let key = (mechanism, identity.purpose, value);
            if let Some(other) = held.insert(key, name)
                && other != name
            {
                problems.push(format!(
                    "the {mechanism} identity '{value}' for {} is held by the Parties \
                     '{other}' and '{name}'",
                    identity.purpose
                ));
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::{PartyConfiguration, PartyIdentity, parties, problems};
    use xcore::{PartyId, Purpose};

    /// Two Parties named here and nowhere else, as a test's own data: the
    /// second holds a key it is sent by and one it presents.
    fn two() -> String {
        let [first, second] = ["first", "second"].map(|word| format!("party-{word}"));
        let id = || PartyConfiguration::new("x", Vec::new()).id.expect("minted");
        let held = |mechanism: &str, purpose: &str, value: &str| {
            format!(
                "  {{ mechanism = \"{mechanism}\", purpose = \"{purpose}\", \
                 value = \"{value}\" }},\n"
            )
        };
        format!(
            "[[parties]]\nid = \"{}\"\nname = \"{first}\"\nidentities = [\n{}]\n\n\
             [[parties]]\nid = \"{}\"\nname = \"{second}\"\nidentities = [\n{}{}{}]\n",
            id(),
            held("mutual-tls", "receive", &format!("CN={first}.example")),
            id(),
            held("mutual-tls", "receive", &format!("CN={second}.example")),
            held("api-key", "receive", "k"),
            held("api-key", "send", "k"),
        )
    }

    #[test]
    fn the_parties_read_in_order_with_their_ids_and_identities() {
        let read = parties(&two()).expect("parses");
        assert_eq!(read.len(), 2);
        assert_eq!(
            read[0].identities,
            [PartyIdentity::new(
                "mutual-tls",
                Purpose::Receive,
                format!("CN={}.example", read[0].name)
            )]
        );
        assert_eq!(read[1].identities.len(), 3);
        assert!(
            read.iter()
                .all(|party| party.id.is_some_and(super::is_uuid_v7))
        );
        assert!(problems(&read).is_empty(), "{:?}", problems(&read));
        assert_eq!(
            parties("[service]\nname = \"n\"\n").expect("none"),
            [] as [PartyConfiguration; 0]
        );
    }

    #[test]
    fn a_party_without_identities_an_unknown_key_or_purpose_is_refused() {
        let error = parties("[[parties]]\nname = \"p\"\n").expect_err("identities");
        assert!(error.contains("identities"), "{error}");
        let unknown = parties("[[parties]]\nname = \"p\"\nidentities = []\nrole = \"x\"\n");
        assert!(unknown.is_err());
        let process = parties(
            "[[parties]]\nname = \"p\"\nidentities = [\
             { mechanism = \"kerberos\", purpose = \"process\", value = \"v\" }]\n",
        )
        .expect_err("process");
        assert!(
            process.contains("is for 'receive' or 'send', not 'process'"),
            "{process}"
        );
        let bare = parties("[[parties]]\nname = \"p\"\nidentities = [\"CN=p\"]\n");
        assert!(
            bare.is_err(),
            "an identity states its mechanism and purpose"
        );
    }

    #[test]
    fn a_party_without_an_id_or_with_one_not_a_uuid_v7_is_refused_in_words() {
        let unnamed = parties(
            "[[parties]]\nname = \"p\"\nidentities = [\
             { mechanism = \"api-key\", purpose = \"receive\", value = \"k\" }]\n",
        )
        .expect("parses");
        let said = problems(&unnamed).join("\n");
        assert!(said.contains("the Party 'p' has no id"), "{said}");

        let mut counted = unnamed.clone();
        counted[0].id = Some(PartyId::new(1));
        let said = problems(&counted).join("\n");
        assert!(said.contains("which is not a UUIDv7"), "{said}");
    }

    #[test]
    fn a_name_or_id_twice_an_empty_party_and_a_shared_identity_are_problems() {
        let mut read = parties(&two()).expect("parses");
        let mut again = read[0].clone();
        again.identities.clone_from(&read[1].identities);
        read.push(again);
        read.push(PartyConfiguration {
            id: None,
            name: String::new(),
            identities: Vec::new(),
        });
        let said = problems(&read).join("\n");
        assert!(said.contains("declared twice"), "{said}");
        assert!(said.contains("have one id"), "{said}");
        assert!(said.contains("is held by the Parties"), "{said}");
        assert!(said.contains("requires a name"), "{said}");
        assert!(said.contains("names no identity"), "{said}");
    }

    #[test]
    fn an_identity_resolves_by_its_mechanism_and_purpose_to_the_party_holding_it() {
        let read = parties(&two()).expect("parses");
        let second = &read[1];

        assert_eq!(
            super::holding(&read, "api-key", Purpose::Receive, "k"),
            second.id
        );
        assert_eq!(super::holding(&read, "jwt", Purpose::Receive, "k"), None);
        assert_eq!(
            super::holding(&read, "api-key", Purpose::Process, "k"),
            None
        );
        let subject = format!("CN={}.example", read[0].name);
        assert_eq!(
            super::holding(&read, "mutual-tls", Purpose::Receive, &subject),
            read[0].id
        );
        assert_eq!(super::named(&read, &second.name), second.id);
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
    fn the_designer_offers_an_identitys_mechanism_value_and_two_purposes() {
        let keys = crate::keys::keys();
        let of_parties =
            |key: &&crate::keys::Key| key.table.first().is_some_and(|t| t == "parties");
        let named = |name: &str| {
            keys.iter()
                .filter(of_parties)
                .find(|key| key.key == name)
                .unwrap_or_else(|| panic!("a Party reads {name}"))
        };
        let purpose = named("purpose");
        assert_eq!(purpose.kind, "choice");
        assert_eq!(purpose.choices, ["receive", "send"]);
        assert_eq!(named("mechanism").kind, "text");
        assert_eq!(named("value").kind, "text");
        let id = keys
            .iter()
            .find(|key| key.key == "id" && key.table == ["parties"])
            .expect("a Party reads its id");
        assert!(
            !id.required,
            "absent, it is refused in words, not by the reader"
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
