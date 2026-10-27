//! `[[correlation_sets]]`: what work is spread over, declared once for the
//! whole cluster (ADR-0024 and ADR-0066, amendments 2026-10-22).
//!
//! The owner, 2026-10-22: *Correlations should be definable, like in
//! BizTalk*; *As long as the correlation sets are cluster wide*; *A default
//! correlation set is its receive location name. … If I want to open up
//! for parallelism I can use context values like Receive Port, Work
//! Process, Send Port, Message and Journey context values. I can widen and
//! strangle parallelism at any configurable point*; and *Equal values in a
//! correlation set (aka hashtable) go in the same queue, different values
//! go parallel depending on the value in the correlation set.*
//!
//! ```toml
//! [[correlation_sets]]
//! name = "PerClient"
//! keys = ["context:Client"]     # the values work is spread over, in order
//! on_failure = "block"          # or "skip"; block where it says nothing
//!
//! [[xmip_applications.receive_locations]]
//! name = "OrdersIn"
//! correlation_set = "PerClient" # each client's orders in order, clients side by side
//! ```
//!
//! A set is named at a point — a Receive Location, a Receive Port, a Work
//! Process or a Send Port — by `correlation_set`, and a Work Process's
//! design names the sets its steps take Messages by. A Receive Location
//! that names none goes one Stream at a time, keyed by its own name.

use serde::{Deserialize, Serialize};

use crate::OnFailure;

/// One correlation set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrelationSet {
    pub name: String,
    /// What makes a key, in order: [`KeyWord`]s.
    pub keys: Vec<String>,
    /// What a Journey that failed does to its sequence: block where it says
    /// nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<OnFailure>,
}

/// What one key of a correlation set reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyWord {
    /// The Receive Location the Journey's Stream came through.
    ReceiveLocation,
    /// Its Receive Port.
    ReceivePort,
    /// The Work Process it is bound for or in.
    WorkProcess,
    /// The Send Port it is bound for.
    SendPort,
    /// A Message Context value, promoted properties among them, by the
    /// name a filter reads it by.
    Context(String),
    /// A Journey Context value: what the Journey itself carries.
    Journey(JourneyValue),
}

/// A Journey Context value a key may read, `journey:<name>` (the owner,
/// 2026-10-27).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JourneyValue {
    /// The Message its Stream was received as, which every Journey born of
    /// one inbound Stream shares: `journey:origin`.
    Origin,
    /// The Subscription that opened it: `journey:subscription`.
    Subscription,
    /// The Journey it came from: `journey:previous`.
    Previous,
}

impl JourneyValue {
    /// Every one, in the order a person reads them.
    pub const ALL: [Self; 3] = [Self::Origin, Self::Subscription, Self::Previous];

    /// Its name after `journey:`.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Origin => "origin",
            Self::Subscription => "subscription",
            Self::Previous => "previous",
        }
    }
}

/// The key words that name an artifact, in the order a person reads them.
pub const ARTIFACT_WORDS: [&str; 4] = [
    "receive-location",
    "receive-port",
    "work-process",
    "send-port",
];

impl KeyWord {
    /// The key word `text` is: a word of [`ARTIFACT_WORDS`],
    /// `journey:<name>` — a [`JourneyValue`] — or `context:<name>` — or
    /// any name — a Message Context value.
    ///
    /// # Errors
    /// It is empty, or names a Journey Context value there is none of.
    pub fn read(text: &str) -> Result<Self, String> {
        Ok(match text.trim() {
            "" => return Err("a correlation set's key is empty".to_string()),
            "receive-location" => Self::ReceiveLocation,
            "receive-port" => Self::ReceivePort,
            "work-process" => Self::WorkProcess,
            "send-port" => Self::SendPort,
            journey if journey.starts_with("journey:") => {
                let name = &journey["journey:".len()..];
                let value = JourneyValue::ALL.into_iter().find(|one| one.word() == name);
                let Some(value) = value else {
                    let known: Vec<String> = JourneyValue::ALL
                        .iter()
                        .map(|one| format!("journey:{}", one.word()))
                        .collect();
                    return Err(format!(
                        "'{journey}' names no Journey Context value; there are {}",
                        known.join(", ")
                    ));
                };
                Self::Journey(value)
            }
            name => Self::Context(name.strip_prefix("context:").unwrap_or(name).to_string()),
        })
    }

    /// The name a filter reads it by, where it is a Message Context value.
    #[must_use]
    pub fn context(&self) -> Option<&str> {
        match self {
            Self::Context(name) => Some(name),
            _ => None,
        }
    }

    /// How a person names it.
    #[must_use]
    pub fn shown(&self) -> String {
        match self {
            Self::ReceiveLocation => "Receive Location".to_string(),
            Self::ReceivePort => "Receive Port".to_string(),
            Self::WorkProcess => "Work Process".to_string(),
            Self::SendPort => "Send Port".to_string(),
            Self::Context(name) => name.clone(),
            Self::Journey(value) => format!("journey:{}", value.word()),
        }
    }
}

/// What is wrong with the correlation sets `sets` and the points naming
/// them, `named`, each a point's kind and name and the set it names: one
/// sentence each.
#[must_use]
pub fn problems(sets: &[CorrelationSet], named: &[(&str, &str, &str)]) -> Vec<String> {
    let mut problems = Vec::new();
    for (place, set) in sets.iter().enumerate() {
        if set.name.trim().is_empty() {
            problems.push("a correlation set requires a name".to_string());
        }
        if sets[..place].iter().any(|earlier| earlier.name == set.name) {
            problems.push(format!(
                "the correlation set '{}' is declared twice",
                set.name
            ));
        }
        if set.keys.is_empty() {
            problems.push(format!("the correlation set '{}' names no key", set.name));
        }
        for key in &set.keys {
            if let Err(why) = KeyWord::read(key) {
                problems.push(format!("the correlation set '{}': {why}", set.name));
            }
        }
    }
    for (kind, name, set) in named {
        if !sets.iter().any(|declared| declared.name == *set) {
            problems.push(format!(
                "the {kind} '{name}' names the correlation set '{set}', which the cluster \
                 does not declare"
            ));
        }
    }
    problems
}

/// What is wrong with the correlation sets `sets` and the points naming
/// them in `bound` and among `work_processes` (each a Work Process's name
/// and the set it names), as [`problems`] says it.
#[must_use]
pub fn bound_problems(
    sets: &[CorrelationSet],
    bound: &crate::Bound,
    work_processes: &[(String, String)],
) -> Vec<String> {
    let named = named_points(bound, work_processes);
    let named: Vec<(&str, &str, &str)> = named
        .iter()
        .map(|(kind, name, set)| (*kind, name.as_str(), set.as_str()))
        .collect();
    problems(sets, &named)
}

/// Every point of `bound` naming a correlation set, and each Work Process
/// of `work_processes`: the point's kind, its name and the set.
fn named_points(
    bound: &crate::Bound,
    work_processes: &[(String, String)],
) -> Vec<(&'static str, String, String)> {
    let mut named = Vec::new();
    for port in &bound.receive_ports {
        if let Some(set) = &port.correlation_set {
            named.push(("Receive Port", port.name.clone(), set.clone()));
        }
        for location in &port.receive_locations {
            if let Some(set) = &location.correlation_set {
                named.push(("Receive Location", location.name.clone(), set.clone()));
            }
        }
    }
    for port in &bound.designed_send_ports {
        if let Some(set) = &port.correlation_set {
            named.push(("Send Port", port.name.clone(), set.clone()));
        }
    }
    for (name, set) in &bound.send_location_sets {
        named.push(("Send Location", name.clone(), set.clone()));
    }
    for (name, set) in work_processes {
        named.push(("Work Process", name.clone(), set.clone()));
    }
    named
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Sets {
        correlation_sets: Vec<CorrelationSet>,
    }

    #[test]
    fn a_set_reads_its_keys_and_an_undeclared_or_broken_one_is_refused_in_words() {
        let read: Sets = toml::from_str(
            "[[correlation_sets]]\nname = \"PerClient\"\nkeys = [\"context:Client\"]\n\
             [[correlation_sets]]\nname = \"PerClient\"\nkeys = []\n\
             [[correlation_sets]]\nname = \"Steps\"\nkeys = [\"journey:Step\"]\n\
             on_failure = \"skip\"\n",
        )
        .expect("reads");
        assert_eq!(read.correlation_sets[2].on_failure, Some(OnFailure::Skip));
        let problems = problems(
            &read.correlation_sets,
            &[("Receive Location", "OrdersIn", "PerOrder")],
        );
        for words in [
            "declared twice",
            "'PerClient' names no key",
            "Journey Context value",
            "'OrdersIn' names the correlation set 'PerOrder'",
        ] {
            assert!(
                problems.iter().any(|p| p.contains(words)),
                "{words}: {problems:?}"
            );
        }
        assert_eq!(
            KeyWord::read("context:Client"),
            Ok(KeyWord::Context("Client".into()))
        );
        assert_eq!(KeyWord::read("send-port"), Ok(KeyWord::SendPort));
    }

    #[test]
    fn a_journey_context_value_is_origin_subscription_or_previous() {
        for value in JourneyValue::ALL {
            let word = format!("journey:{}", value.word());
            assert_eq!(KeyWord::read(&word), Ok(KeyWord::Journey(value)));
            assert_eq!(KeyWord::Journey(value).shown(), word);
        }
        let refused = KeyWord::read("journey:depth").expect_err("no such value");
        assert!(
            refused.contains("journey:origin, journey:subscription, journey:previous"),
            "{refused}"
        );
    }
}
