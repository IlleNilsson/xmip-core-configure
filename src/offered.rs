//! What a designer offers to set on an entry (ADR-0064, amendment
//! 2026-10-22): every key its table may hold and does not write yet, with
//! its kind, whether it is required, and the words or names it may take.
//!
//! The keys are this crate's readers' own ([`crate::keys`]); a reference's
//! names are what the file declares where it looks ([`crate::references`]) —
//! a Party for a Location's `accept`, the calls a Port or Location may
//! attach at its stage, the units of a module that fit a call; and the
//! words of what this crate does not own — `[tuning]`'s keys and the role
//! words — are handed in by the runtime that owns them
//! ([`crate::views::Views::offering`]). The VS Code designer and the
//! Operation Desktop's Configure page both draw this list and hold no rule.

use std::collections::BTreeSet;

use serde::Serialize;
use xcore::settings::Settings;

use crate::keys::{Key, of_table};
use crate::references::Reference;
use crate::views::{Entry, Views};
use crate::written::Written;

/// One key an entry may be given.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Offered {
    pub key: String,
    /// As the key's reader says it: `text`, `boolean`, `choice`, `roles`,
    /// … or, in `[tuning]`, the declaration's kind.
    pub kind: &'static str,
    pub required: bool,
    /// The words or names its value may take; empty where any value of
    /// its kind may be written.
    pub choices: Vec<String>,
    /// What it means, where its declaration says.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meaning: Option<&'static str>,
    /// For an array of unnamed tables, such as a Party's `identities`: the
    /// keys one element holds, each added together as one element.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<Offered>,
}

/// What `entry` may be given, in the file whose tables are `written` and
/// whose names are `references`.
pub(crate) fn offered(
    entry: &Entry,
    written: &[Written],
    references: &[Reference],
    tuning: Option<&Settings>,
    roles: &[&str],
) -> Vec<Offered> {
    let set: BTreeSet<&str> = entry
        .fields
        .iter()
        .filter_map(|field| field.key.first().map(String::as_str))
        .collect();
    let table = written
        .iter()
        .find(|table| table.section == entry.section)
        .map_or_else(|| entry.section.clone(), |table| table.table.clone());

    if table.last().map(String::as_str) == Some("tuning") {
        return tuning
            .into_iter()
            .flat_map(|declared| declared.settings.iter())
            .filter(|setting| !set.contains(setting.name))
            .map(|setting| Offered {
                key: setting.name.to_string(),
                kind: setting.kind.word(),
                required: false,
                choices: Vec::new(),
                meaning: Some(setting.meaning),
                elements: Vec::new(),
            })
            .collect();
    }

    of_table(&table)
        .into_iter()
        .filter_map(|key| {
            let mut elements = Vec::new();
            if key.kind == "list" {
                // A list of entries, each named, is added as an entry; a list
                // of unnamed tables is added to element by element.
                let within = [table.clone(), vec![key.key.clone()]].concat();
                let held = of_table(&within);
                if held
                    .iter()
                    .any(|inner| crate::field::NAMED_BY.contains(&inner.key.as_str()))
                {
                    return None;
                }
                elements = held
                    .into_iter()
                    .map(|inner| Offered {
                        key: inner.key.clone(),
                        kind: inner.kind,
                        required: inner.required,
                        choices: inner.choices.iter().map(ToString::to_string).collect(),
                        meaning: inner.meaning,
                        elements: Vec::new(),
                    })
                    .collect();
            } else if set.contains(key.key.as_str()) {
                return None;
            }
            Some(Offered {
                key: key.key.clone(),
                kind: key.kind,
                required: key.required,
                choices: choices(entry, key, references, roles),
                meaning: key.meaning,
                elements,
            })
        })
        .collect()
}

/// The names a reference at `key` of `entry` may take, its words, or the
/// role words.
fn choices(entry: &Entry, key: &Key, references: &[Reference], roles: &[&str]) -> Vec<String> {
    let named: BTreeSet<&String> = references
        .iter()
        .filter(|reference| {
            reference.holder == entry.section && reference.key.first() == Some(&key.key)
        })
        .flat_map(|reference| &reference.choices)
        .collect();
    if !named.is_empty() {
        return named.into_iter().cloned().collect();
    }
    let words: &[&str] = if key.kind == "roles" {
        roles
    } else {
        &key.choices
    };
    words.iter().map(ToString::to_string).collect()
}

impl Views {
    /// The views with each entry given what it may be set: `tuning` the
    /// runtime's declaration of `[tuning]`, `roles` the role words.
    #[must_use]
    pub fn offering(mut self, tuning: Option<&Settings>, roles: &[&str]) -> Self {
        for view in &mut self.views {
            for entry in &mut view.entries {
                entry.offered = offered(entry, &self.written, &self.references, tuning, roles);
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::views::Views;
    use crate::views::tests::written;

    #[test]
    fn an_entry_is_offered_what_it_does_not_write_with_its_words_and_names() {
        let (text, first, _) = written();
        let text = format!(
            "{text}\n[[parties]]\nid = \"0192f000-0000-7000-8000-000000000001\"\n\
             name = \"partner\"\nidentities = []\n"
        );
        let views = Views::of(&text)
            .expect("reads")
            .offering(None, &["executing"]);
        let entry = |kind: &str, name: &str| {
            let view = views
                .views
                .iter()
                .find(|view| view.kind == kind)
                .expect(kind);
            view.entries
                .iter()
                .find(|entry| entry.name == name)
                .expect(name)
                .clone()
        };

        let drop = entry("receive-location", "drop");
        let keys: Vec<_> = drop.offered.iter().map(|o| o.key.as_str()).collect();
        assert!(
            keys.contains(&"secret") && !keys.contains(&"address"),
            "{keys:?}"
        );
        let secret = drop.offered.iter().find(|o| o.key == "secret");
        assert!(secret.is_some_and(|o| o.meaning.is_some()), "its meaning");
        let node = entry("node", &first);
        let roles = node
            .offered
            .iter()
            .find(|o| o.key == "roles")
            .expect("roles");
        assert_eq!(roles.choices, ["executing"]);
        let location = entry("receive-location", "OrdersIn");
        let depth = location.offered.iter().find(|o| o.key == "depth");
        assert!(depth.is_none(), "written");
    }
}
