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
}

/// The key words that name an artifact, in the order a person reads them.
pub const ARTIFACT_WORDS: [&str; 4] = [
    "receive-location",
    "receive-port",
    "work-process",
    "send-port",
];

impl KeyWord {
    /// The key word `text` is: a word of [`ARTIFACT_WORDS`], or
    /// `context:<name>` — or any name — a Message Context value.
    ///
    /// # Errors
    /// It is empty, or names a Journey Context value, which nothing sets
    /// yet.
    pub fn read(text: &str) -> Result<Self, String> {
        Ok(match text.trim() {
            "" => return Err("a correlation set's key is empty".to_string()),
            "receive-location" => Self::ReceiveLocation,
            "receive-port" => Self::ReceivePort,
            "work-process" => Self::WorkProcess,
            "send-port" => Self::SendPort,
            journey if journey.starts_with("journey:") => {
                return Err(format!(
                    "'{journey}' names a Journey Context value, which nothing sets yet"
                ));
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
}
