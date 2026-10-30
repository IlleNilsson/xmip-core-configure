//! `[[correlations]]`: what work is spread over, declared once for the
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
//! [[correlations]]
//! name = "PerClient"
//! keys = ["context:Client"]     # the values work is spread over, in order
//! on_failure = "block"          # or "skip"; block where it says nothing
//!
//! [[xmip_applications.receive_locations]]
//! name = "OrdersIn"
//! correlation = "PerClient" # each client's orders in order, clients side by side
//! ```
//!
//! A set is named at a point — a Receive Location, a Receive Port, a Work
//! Process, a Send Port Group, a Send Port or a Send Location — by
//! `correlation`, and a Work Process's
//! design names the sets its steps take Messages by. A Receive Location
//! that names none goes one Stream at a time, keyed by its own name.

mod crossing;

use serde::{Deserialize, Serialize};

use crate::OnFailure;
pub use crossing::crossing;

/// One Correlation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Correlation {
    pub name: String,
    /// What makes a key, in order: [`KeyWord`]s.
    pub keys: Vec<String>,
    /// What a Message that failed does to its sequence: block where it says
    /// nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<OnFailure>,
}

/// What one key of a Correlation reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyWord {
    /// The Receive Location the Journey's Stream came through.
    ReceiveLocation,
    /// Its Receive Port.
    ReceivePort,
    /// The Work Process it is bound for or in.
    WorkProcess,
    /// The Send Port Group it was spread by.
    SendPortGroup,
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
pub const ARTIFACT_WORDS: [&str; 5] = [
    "receive-location",
    "receive-port",
    "work-process",
    "send-port-group",
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
            "" => return Err("a Correlation's key is empty".to_string()),
            "receive-location" => Self::ReceiveLocation,
            "receive-port" => Self::ReceivePort,
            "work-process" => Self::WorkProcess,
            "send-port-group" => Self::SendPortGroup,
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
            Self::SendPortGroup => "Send Port Group".to_string(),
            Self::SendPort => "Send Port".to_string(),
            Self::Context(name) => name.clone(),
            Self::Journey(value) => format!("journey:{}", value.word()),
        }
    }
}

/// What is wrong with the Correlations `sets` and the points naming
/// them, `named`, each a point's kind and name and the set it names: one
/// sentence each.
#[must_use]
pub fn problems(sets: &[Correlation], named: &[(&str, &str, &str)]) -> Vec<String> {
    let mut problems = Vec::new();
    for (place, set) in sets.iter().enumerate() {
        if set.name.trim().is_empty() {
            problems.push("a Correlation requires a name".to_string());
        }
        if sets[..place].iter().any(|earlier| earlier.name == set.name) {
            problems.push(format!("the Correlation '{}' is declared twice", set.name));
        }
        if set.keys.is_empty() {
            problems.push(format!("the Correlation '{}' names no key", set.name));
        }
        for key in &set.keys {
            if let Err(why) = KeyWord::read(key) {
                problems.push(format!("the Correlation '{}': {why}", set.name));
            }
        }
    }
    for (kind, name, set) in named {
        if !sets.iter().any(|declared| declared.name == *set) {
            problems.push(format!(
                "the {kind} '{name}' names the Correlation '{set}', which the cluster \
                 does not declare"
            ));
        }
    }
    problems
}

/// What is wrong with the Correlations `sets` and the points naming
/// them in `bound` and among `work_processes` (each a Work Process's name
/// and the set it names), as [`problems`] says it, and sets taken in
/// crossing order ([`crossing`]).
#[must_use]
pub fn bound_problems(
    sets: &[Correlation],
    bound: &crate::Bound,
    work_processes: &[(String, String)],
) -> Vec<String> {
    let named = named_points(bound, work_processes);
    let named: Vec<(&str, &str, &str)> = named
        .iter()
        .map(|(kind, name, set)| (*kind, name.as_str(), set.as_str()))
        .collect();
    let mut problems = problems(sets, &named);
    problems.extend(crossing(bound, work_processes));
    problems
}

/// Every point of `bound` naming a Correlation, and each Work Process
/// of `work_processes`: the point's kind, its name and the set.
fn named_points(
    bound: &crate::Bound,
    work_processes: &[(String, String)],
) -> Vec<(&'static str, String, String)> {
    let mut named = Vec::new();
    for port in &bound.receive_ports {
        if let Some(set) = &port.correlation {
            named.push(("Receive Port", port.name.clone(), set.clone()));
        }
        for location in &port.receive_locations {
            if let Some(set) = &location.correlation {
                named.push(("Receive Location", location.name.clone(), set.clone()));
            }
        }
    }
    for group in &bound.send_port_groups {
        if let Some(set) = &group.correlation {
            named.push(("Send Port Group", group.name.clone(), set.clone()));
        }
    }
    for port in &bound.designed_send_ports {
        if let Some(set) = &port.correlation {
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

/// Every Send Location of `document` naming a Correlation, its own and
/// every binding's whichever node takes it: its name and the set.
#[must_use]
pub fn send_location_sets(document: &crate::XmipConfigurationDocument) -> Vec<(String, String)> {
    let bound = document
        .applications
        .iter()
        .flat_map(|binding| binding.send_ports.iter().map(|bound| &bound.location));
    document
        .send_locations
        .iter()
        .chain(bound)
        .filter_map(|location| {
            let set = location.correlation.clone()?;
            Some((location.name.clone(), set))
        })
        .collect()
}

/// Every Receive Location of `document`, its own and every binding's, that
/// names a Correlation: by its name. A Receive Location names its set
/// in its Application's design, never here.
#[must_use]
pub fn receive_location_sets(document: &crate::XmipConfigurationDocument) -> Vec<String> {
    let bound = document.applications.iter().flat_map(|binding| {
        binding
            .receive_locations
            .iter()
            .map(|bound| &bound.location)
    });
    document
        .receive_locations
        .iter()
        .chain(bound)
        .filter(|location| location.correlation.is_some())
        .map(|location| location.name.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Sets {
        correlations: Vec<Correlation>,
    }

    #[test]
    fn a_set_reads_its_keys_and_an_undeclared_or_broken_one_is_refused_in_words() {
        let read: Sets = toml::from_str(
            "[[correlations]]\nname = \"PerClient\"\nkeys = [\"context:Client\"]\n\
             [[correlations]]\nname = \"PerClient\"\nkeys = []\n\
             [[correlations]]\nname = \"Steps\"\nkeys = [\"journey:Step\"]\n\
             on_failure = \"skip\"\n",
        )
        .expect("reads");
        assert_eq!(read.correlations[2].on_failure, Some(OnFailure::Skip));
        let problems = problems(
            &read.correlations,
            &[("Receive Location", "OrdersIn", "PerOrder")],
        );
        for words in [
            "declared twice",
            "'PerClient' names no key",
            "Journey Context value",
            "'OrdersIn' names the Correlation 'PerOrder'",
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
        assert_eq!(KeyWord::read("send-port-group"), Ok(KeyWord::SendPortGroup));
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
