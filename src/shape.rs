//! What a reader takes, read from the reader's own declaration: the keys a
//! `Deserialize` type asks for, the kind of value each is, the words an
//! enum reads and the keys it refuses to go without (ADR-0064, amendment
//! 2026-10-13: the editor suggests every key from the one parse).
//!
//! Nothing here lists a key. [`shapes`] hands the type a deserializer that
//! answers every question the type asks with a value of the kind it asked
//! for, and writes down what was asked: a struct names its fields, an enum
//! its words, a missing field the one it requires. The type is the one
//! declaration; this only listens to it. A part a type reads through
//! `flatten` or `deserialize_any` asks no named question and is read as a
//! table of free keys.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, DeserializeOwned};

use crate::probe::Probe;

/// What a reader takes at one key path.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shape {
    /// `text`, `integer`, `float`, `boolean`, `choice` (an enum's word),
    /// `variant` (a one-key table naming an enum's case), `table` (a
    /// struct's keys), `list` (a list of tables), `array` (of values),
    /// `free` (a table of keys the type does not name), or `any`.
    pub kind: &'static str,
    /// A table's keys, in the order the type declares them.
    pub fields: Vec<&'static str>,
    /// An enum's words.
    pub choices: Vec<&'static str>,
    /// The keys the type refuses to read without.
    pub required: Vec<&'static str>,
}

/// Every path the type reads, its keys from the top; a list's entries share
/// the list's path.
pub type Shapes = BTreeMap<Vec<String>, Shape>;

/// The shapes `T` reads.
#[must_use]
pub fn shapes<T: DeserializeOwned>() -> Shapes {
    let state = RefCell::new(State::default());
    let run = |state: &RefCell<State>| {
        let _ = T::deserialize(Probe {
            path: Vec::new(),
            element: false,
            state,
        });
    };
    run(&state);

    // A field whose value refused what it was given stops its table's
    // reading, so the fields after it are each read on their own — every
    // table above it given only the way down to it, so a field an outer
    // table refuses before it does not stop the reading short of it.
    loop {
        let unread = unread(&state.borrow().shapes);
        if unread.is_empty() {
            break;
        }
        for (table, field) in unread {
            let alone = alone(&state.borrow().shapes, &table, field);
            state.borrow_mut().given.extend(alone.iter().cloned());
            run(&state);
            let mut held = state.borrow_mut();
            for (path, _) in &alone {
                held.given.remove(path);
            }
            let child = [table, vec![field.to_string()]].concat();
            held.shapes.entry(child).or_insert_with(|| Shape {
                kind: "any",
                ..Shape::default()
            });
        }
    }

    // What a table requires, one field at a time: given only what it is
    // known to require, and every table above it only the way down to it,
    // it names the next one it goes without.
    let tables: Vec<Vec<String>> = state
        .borrow()
        .shapes
        .iter()
        .filter(|(_, shape)| !shape.fields.is_empty())
        .map(|(path, _)| path.clone())
        .collect();
    for table in tables {
        let way: Vec<_> = table
            .split_last()
            .map(|(last, above)| {
                let shapes = &state.borrow().shapes;
                let field = shapes
                    .get(above)
                    .and_then(|shape| shape.fields.iter().find(|key| *key == last).copied());
                field.map_or_else(Vec::new, |field| alone(shapes, above, field))
            })
            .unwrap_or_default();
        loop {
            let required = state.borrow().shapes[&table].required.clone();
            let mut held = state.borrow_mut();
            held.given.extend(way.iter().cloned());
            held.given.insert(table.clone(), required.clone());
            drop(held);
            run(&state);
            let mut held = state.borrow_mut();
            match held.missing.take() {
                Some((at, field)) if at == table && !required.contains(&field) => {
                    if let Some(shape) = held.shapes.get_mut(&table) {
                        shape.required.push(field);
                    }
                }
                _ => break,
            }
        }
        let mut held = state.borrow_mut();
        for (path, _) in &way {
            held.given.remove(path);
        }
        held.given.remove(&table);
    }

    state.into_inner().shapes
}

/// What each table on the way to `field` of `table` is given to read it
/// alone: the table that field, and each table above it the key leading on.
fn alone(
    shapes: &Shapes,
    table: &[String],
    field: &'static str,
) -> Vec<(Vec<String>, Vec<&'static str>)> {
    let mut given = vec![(table.to_vec(), vec![field])];
    for depth in 0..table.len() {
        let above = table[..depth].to_vec();
        let on = shapes
            .get(&above)
            .and_then(|shape| shape.fields.iter().find(|key| **key == table[depth]));
        if let Some(on) = on {
            given.push((above, vec![*on]));
        }
    }
    given
}

/// The fields whose value no reading has reached yet.
fn unread(shapes: &Shapes) -> Vec<(Vec<String>, &'static str)> {
    shapes
        .iter()
        .flat_map(|(path, shape)| shape.fields.iter().map(move |field| (path, *field)))
        .filter(|(path, field)| {
            let child = [(*path).clone(), vec![(*field).to_string()]].concat();
            !shapes.contains_key(&child)
        })
        .map(|(path, field)| (path.clone(), field))
        .collect()
}

#[derive(Default)]
pub(crate) struct State {
    pub(crate) shapes: Shapes,
    /// The fields a table is given, where not every one.
    pub(crate) given: BTreeMap<Vec<String>, Vec<&'static str>>,
    /// The field a table said it requires, and the table.
    pub(crate) missing: Option<(Vec<String>, &'static str)>,
}

/// Why a reading stopped: the field a table requires, until the table that
/// requires it has said so.
#[derive(Debug)]
pub(crate) struct Stop(pub(crate) Option<&'static str>);

impl fmt::Display for Stop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "stopped")
    }
}

impl std::error::Error for Stop {}

impl de::Error for Stop {
    fn custom<T: fmt::Display>(_: T) -> Self {
        Self(None)
    }

    fn missing_field(field: &'static str) -> Self {
        Self(Some(field))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::XmipConfigurationDocument;
    use crate::application::XmipApplication;

    fn path(steps: &[&str]) -> Vec<String> {
        steps.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn a_node_document_says_its_tables_keys_kinds_and_requirements() {
        let shapes = shapes::<XmipConfigurationDocument>();

        let root = &shapes[&Vec::new()];
        assert!(root.fields.contains(&"receive_locations"), "{root:?}");
        assert_eq!(root.required, ["service"]);
        let location = &shapes[&path(&["receive_locations"])];
        assert_eq!(location.kind, "list");
        assert!(!location.fields.contains(&"transport"), "{location:?}");
        for required in ["name", "address"] {
            assert!(location.required.contains(&required), "{location:?}");
        }
        for optional in ["start", "contract"] {
            assert!(!location.required.contains(&optional), "{location:?}");
        }
        assert_eq!(
            shapes[&path(&["receive_locations", "start"])].kind,
            "boolean"
        );
        assert_eq!(
            shapes[&path(&["receive_locations", "settings"])].kind,
            "free"
        );
        for key in ["listen", "certificate", "key", "anchors"] {
            assert_eq!(shapes[&path(&["storage", key])].kind, "text", "{key}");
        }
        assert_eq!(shapes[&path(&["storage", "nodes"])].kind, "array");
    }

    #[test]
    fn an_application_says_its_lists_and_a_destinations_cases() {
        let shapes = shapes::<XmipApplication>();

        let depth = &shapes[&path(&["receive_locations", "depth"])];
        assert!(depth.choices.contains(&"light"), "{depth:?}");
        let destination = &shapes[&path(&["subscriptions", "destination"])];
        assert_eq!(destination.kind, "variant");
        assert!(
            destination.choices.contains(&"send-port"),
            "{destination:?}"
        );
        let subscription = &shapes[&path(&["subscriptions"])];
        assert_eq!(subscription.required, ["id", "destination", "filter"]);
        let retry = &shapes[&path(&["send_ports", "retry"])];
        assert_eq!(retry.fields, ["attempts", "backoff"]);
        assert_eq!(
            shapes[&path(&["send_ports", "retry", "attempts"])].kind,
            "integer"
        );
        assert_eq!(
            shapes[&path(&["send_port_groups", "send_ports"])].kind,
            "array"
        );
    }
}
