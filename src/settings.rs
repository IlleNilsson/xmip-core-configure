//! A Location's settings, as its TOML holds them, and their validation
//! against the declaration its technology makes (ADR-0064, amendment
//! 2026-09-26).
//!
//! ```toml
//! [[receive_locations]]
//! name = "orders"
//! start = true
//! transport = "xmip-core-transport-kafka"
//! address = "broker.example:9092"
//! contract = "xmip-core-contract-json-schema"
//!
//! [receive_locations.settings]
//! topic = "orders"
//!
//! [receive_locations.contract_settings]
//! reference = "schemas/order.json"
//! ```
//!
//! What a setting may be is never this crate's: every technology declares
//! its own (`xcore::settings::Settings`), and [`location_problems`] reads a
//! Location's tables through that declaration — the same reading the
//! technology builds itself with — so what start refuses and what the
//! technology is given cannot disagree. Which technologies there are is not
//! this crate's either: the caller hands over the declarations it carries.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use xcore::settings::{Applies, Given, Settings};

use crate::ConfiguredLocation;

/// A settings table as written: a Location's `settings` or
/// `contract_settings`, or a node's `[tuning]`, each read through the
/// declaration of whoever takes it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LocationSettings(pub toml::Table);

// A table can hold a float, and a float is not Eq. No setting is a float —
// the declaration refuses one — so equality here is the documents' equality
// the configuration model has always derived.
impl Eq for LocationSettings {}

impl LocationSettings {
    /// Whether the Location gave nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The table as a technology's declaration reads it: each value as TOML
    /// wrote it, by its key.
    #[must_use]
    pub fn given(&self) -> Vec<(String, Given)> {
        self.0
            .iter()
            .map(|(name, value)| (name.clone(), given(value)))
            .collect()
    }
}

/// A TOML value as a declaration reads it.
fn given(value: &toml::Value) -> Given {
    match value {
        toml::Value::String(text) => Given::Text(text.clone()),
        toml::Value::Integer(number) => Given::Integer(*number),
        toml::Value::Boolean(flag) => Given::Boolean(*flag),
        toml::Value::Float(_) => Given::Other("float"),
        toml::Value::Datetime(_) => Given::Other("date"),
        toml::Value::Array(_) => Given::Other("array"),
        toml::Value::Table(_) => Given::Other("table"),
    }
}

/// The declarations a caller carries, by technology: what the runtime's
/// catalogue holds.
pub type Declarations = BTreeMap<&'static str, &'static Settings>;

/// What is wrong with `location`'s settings on `side`, one sentence each
/// naming the Location, its technology and the setting: every setting its
/// transport's or contract's declaration refuses, and contract settings
/// given without a contract.
///
/// A technology `declared` does not hold is not judged here: which
/// technologies a node carries is its caller's to say.
#[must_use]
pub fn location_problems(
    location: &ConfiguredLocation,
    side: Applies,
    declared: &Declarations,
) -> Vec<String> {
    let mut problems = Vec::new();
    let name = &location.name;

    if let Some(settings) = declared.get(location.transport.as_str())
        && let Err(refused) = settings.read(side, &location.settings.given())
    {
        for refusal in refused.0 {
            problems.push(format!("the Location '{name}': {refusal}"));
        }
    }

    match &location.contract {
        Some(contract) => {
            if let Some(settings) = declared.get(contract.as_str())
                && let Err(refused) = settings.read(side, &location.contract_settings.given())
            {
                for refusal in refused.0 {
                    problems.push(format!("the Location '{name}': {refusal}"));
                }
            }
        }
        None if !location.contract_settings.is_empty() => problems.push(format!(
            "the Location '{name}' gives contract_settings and names no contract"
        )),
        None => {}
    }

    // ADR-0019 clause 1 and its mirror: a Receive Location declares what it
    // accepts, a Send Location what it presents. Neither reads the other's.
    if side == Applies::Send && !location.accept.is_empty() {
        problems.push(format!(
            "the Send Location '{name}' gives accept, which only a Receive Location reads"
        ));
    }

    problems
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_toml;
    use xcore::settings::{Kind, Presence, Setting};

    const KAFKA: &Settings = &Settings {
        technology: "xmip-core-transport-kafka",
        settings: &[
            Setting {
                name: "topic",
                kind: Kind::Text,
                presence: Presence::Required,
                meaning: "The topic a Location reads or writes.",
                applies: Applies::Both,
            },
            Setting {
                name: "group",
                kind: Kind::Text,
                presence: Presence::Optional,
                meaning: "The consumer group a Receive Location reads in.",
                applies: Applies::Receive,
            },
        ],
    };

    fn declared() -> Declarations {
        BTreeMap::from([(KAFKA.technology, KAFKA)])
    }

    fn location(settings: &str) -> ConfiguredLocation {
        let source = format!(
            "[service]\nname = \"n\"\ncluster_name = \"c\"\nnode_name = \"d\"\n\
             [[send_locations]]\nname = \"out\"\nstart = true\n\
             transport = \"xmip-core-transport-kafka\"\naddress = \"broker:9092\"\n\
             {settings}"
        );
        parse_toml(&source).expect("parses").send_locations[0].clone()
    }

    #[test]
    fn a_location_its_declaration_reads_has_no_problem() {
        let sound = location("[send_locations.settings]\ntopic = \"orders\"\n");
        assert_eq!(sound.settings.given().len(), 1);
        assert!(location_problems(&sound, Applies::Send, &declared()).is_empty());
    }

    #[test]
    fn unknown_wrong_kind_missing_and_other_side_each_name_the_setting() {
        let unsound = location("[send_locations.settings]\ncolour = \"lime\"\ngroup = \"g\"\n");
        let problems = location_problems(&unsound, Applies::Send, &declared());
        assert_eq!(problems.len(), 3, "{problems:?}");
        for word in ["colour", "group", "topic"] {
            assert!(
                problems
                    .iter()
                    .any(|p| p.contains(word) && p.contains("kafka")),
                "{word}: {problems:?}"
            );
        }
        let wrong = location("[send_locations.settings]\ntopic = 7\n");
        let problems = location_problems(&wrong, Applies::Send, &declared());
        assert!(problems[0].contains("as text"), "{problems:?}");
    }

    #[test]
    fn contract_settings_without_a_contract_are_refused() {
        let unsound = location("[send_locations.contract_settings]\nreference = \"a.json\"\n");
        let problems = location_problems(&unsound, Applies::Send, &Declarations::new());
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("names no contract"));
    }

    #[test]
    fn accept_is_read_on_a_receive_location_and_refused_on_a_send_location() {
        let accepting = location("[send_locations.accept]\nmechanism = [\"circumstance\"]\n");
        assert_eq!(accepting.accept.mechanism, ["circumstance"]);
        let problems = location_problems(&accepting, Applies::Send, &Declarations::new());
        assert_eq!(
            problems,
            ["the Send Location 'out' gives accept, which only a Receive Location reads"]
        );
        assert!(location_problems(&accepting, Applies::Receive, &Declarations::new()).is_empty());

        let source = "[service]\nname = \"n\"\ncluster_name = \"c\"\nnode_name = \"d\"\n\
             [[receive_locations]]\nname = \"in\"\nstart = true\ntransport = \"t\"\n\
             address = \"a\"\n[receive_locations.accept]\nparty = [\"party-x\"]\n";
        let refused = parse_toml(source).expect_err("no Party is named in a node yet");
        assert!(refused.contains("party"), "{refused}");
    }

    #[test]
    fn a_technology_the_caller_does_not_carry_is_not_judged_here() {
        let unknown = location("[send_locations.settings]\nanything = 1\n");
        assert!(location_problems(&unknown, Applies::Send, &Declarations::new()).is_empty());
    }

    #[test]
    fn settings_round_trip_through_the_document() {
        let source = "[service]\nname = \"n\"\ncluster_name = \"c\"\nnode_name = \"d\"\n\
             [[receive_locations]]\nname = \"in\"\nstart = true\n\
             transport = \"xmip-core-transport-kafka\"\naddress = \"b:1\"\n\
             contract = \"xmip-core-contract-json-schema\"\n\
             [receive_locations.settings]\ntopic = \"orders\"\n";
        let document = parse_toml(source).expect("parses");
        let written = toml::to_string(&document).expect("writes");
        assert_eq!(parse_toml(&written).expect("reads back"), document);
    }
}
