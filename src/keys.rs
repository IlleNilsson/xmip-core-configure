//! The keys a cluster's `xmip.toml` may hold, table by table, as its
//! readers declare them (ADR-0064, amendment 2026-10-13: every key and value
//! suggested from the one parse, never a copy in a surface).
//!
//! The file is the shared node document at the top
//! ([`crate::XmipConfigurationDocument`]), the same document under each
//! `[nodes.<name>]` with the node's [`crate::cluster::ROLES`], each Xmip
//! Application as its section ([`crate::XmipApplication`]), and each bound
//! Receive Location and Send Port a Location with the node that takes it
//! ([`crate::BoundLocation`]). Every key, kind, word and requirement is read
//! from those types by [`crate::shape`]; this file only says where in the
//! file each type is read.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::binding::BoundLocation;
use crate::cluster::{NODES, ROLES};
use crate::section::SECTIONS;
use crate::shape::{Shape, Shapes, shapes};
use crate::{ConfiguredLocation, LocationSettings, XmipApplication, XmipConfigurationDocument};

/// One key a table of the file may hold.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Key {
    /// The table, by the keys its header writes: `["nodes", "*", "tuning"]`,
    /// `*` standing for any node's name; a list's entries share its path.
    pub table: Vec<String>,
    pub key: String,
    /// As [`Shape::kind`] says it, or `roles`: the role words a node
    /// declares, by comma.
    pub kind: &'static str,
    /// The words a `choice` reads, or the cases a `variant` is keyed by.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<&'static str>,
    /// Whether the table is refused without it.
    pub required: bool,
}

/// What stands for any node's name in [`Key::table`].
pub const ANY_NODE: &str = "*";

/// Every key of the cluster's file, read once.
#[must_use]
pub fn keys() -> &'static [Key] {
    static KEYS: OnceLock<Vec<Key>> = OnceLock::new();
    KEYS.get_or_init(read)
}

/// The keys `table`, written as its header writes it, may hold: a node's
/// name matches [`ANY_NODE`].
#[must_use]
pub fn of_table(table: &[String]) -> Vec<&'static Key> {
    keys()
        .iter()
        .filter(|key| {
            key.table.len() == table.len()
                && key
                    .table
                    .iter()
                    .zip(table)
                    .all(|(pattern, step)| pattern == ANY_NODE || pattern == step)
        })
        .collect()
}

fn read() -> Vec<Key> {
    let document = shapes::<XmipConfigurationDocument>();
    // A node declares its roles under its own table, never the cluster's
    // shared top: the document's [`ROLES`] is the slice's copy of it.
    let declared = |key: &Key, at: usize| key.key == ROLES && key.table.len() == at;
    let mut keys: Vec<Key> = listed(&document, &[])
        .into_iter()
        .filter(|key| !declared(key, 0))
        .collect();

    keys.push(key(&[], NODES, "free", &[], false));
    let node = [NODES, ANY_NODE];
    keys.extend(
        listed(&document, &node)
            .into_iter()
            .filter(|key| !declared(key, node.len())),
    );
    keys.push(key(&node, ROLES, "roles", &[], false));

    let application = shapes::<XmipApplication>();
    keys.extend(listed(&application, &[SECTIONS]));

    let location = shapes::<ConfiguredLocation>();
    for at in [&[][..], &node[..]] {
        for list in ["receive_locations", "send_ports"] {
            let bound = [at, &["applications", list][..]].concat();
            keys.extend(listed(&location, &bound));
            keys.extend(bound_own(
                &location,
                &bound.iter().map(ToString::to_string).collect::<Vec<_>>(),
            ));
        }
    }
    keys
}

/// Each table of `shapes` and its keys, beneath `at`.
fn listed(shapes: &Shapes, at: &[&str]) -> Vec<Key> {
    let mut keys = Vec::new();
    for (path, shape) in shapes {
        let table: Vec<String> = at
            .iter()
            .map(ToString::to_string)
            .chain(path.iter().cloned())
            .collect();
        for field in &shape.fields {
            let child = [path.clone(), vec![(*field).to_string()]].concat();
            let held = shapes.get(&child).cloned().unwrap_or_default();
            keys.push(Key {
                table: table.clone(),
                key: (*field).to_string(),
                kind: kind_of(&held),
                choices: held.choices,
                required: shape.required.contains(field),
            });
        }
    }
    keys
}

fn kind_of(shape: &Shape) -> &'static str {
    if shape.kind.is_empty() {
        "any"
    } else {
        shape.kind
    }
}

fn key(
    table: &[&str],
    key: &str,
    kind: &'static str,
    choices: &[&'static str],
    required: bool,
) -> Key {
    Key {
        table: table.iter().map(ToString::to_string).collect(),
        key: key.to_string(),
        kind,
        choices: choices.to_vec(),
        required,
    }
}

/// The keys a bound Location holds beside the Location it flattens: what a
/// [`BoundLocation`] writes that a [`ConfiguredLocation`] does not, each
/// required when the binding does not read without it. Read by writing one
/// and reading it back, since a flattened type names no field of its own.
fn bound_own(location: &Shapes, table: &[String]) -> Vec<Key> {
    let configured = &location
        .get(&Vec::new())
        .cloned()
        .unwrap_or_default()
        .fields;
    let one = BoundLocation {
        node: String::new(),
        location: ConfiguredLocation {
            name: String::new(),
            start: false,
            address: String::new(),
            secret: None,
            contract: None,
            settings: LocationSettings::default(),
            contract_settings: LocationSettings::default(),
            accept: crate::Accept::default(),
            prepare: Vec::new(),
            transform: None,
            promote: None,
            demote: None,
            debatch: None,
            correlation: None,
        },
    };
    let Ok(toml::Value::Table(written)) = toml::Value::try_from(&one) else {
        return Vec::new();
    };
    written
        .iter()
        .filter(|(name, _)| !configured.contains(&name.as_str()))
        .map(|(name, value)| {
            let mut without = written.clone();
            without.remove(name);
            Key {
                table: table.to_vec(),
                key: name.clone(),
                kind: if value.is_str() { "text" } else { "any" },
                choices: Vec::new(),
                required: BoundLocation::deserialize(toml::Value::Table(without)).is_err(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(steps: &[&str]) -> Vec<String> {
        steps.iter().map(ToString::to_string).collect()
    }

    fn named<'k>(table: &[&str], name: &str) -> &'k Key {
        of_table(&path(table))
            .into_iter()
            .find(|key| key.key == name)
            .unwrap_or_else(|| panic!("no {name} in {table:?}"))
    }

    #[test]
    fn every_table_of_the_file_has_its_readers_keys() {
        let node = crate::fixture::test_cluster().node(0).name.clone();

        let start = named(&["receive_locations"], "start");
        assert!(
            start.kind == "boolean" && !start.required,
            "nothing: started"
        );
        assert!(named(&["receive_locations"], "address").required);
        assert!(
            of_table(&path(&["receive_locations"]))
                .iter()
                .all(|key| key.key != "transport"),
            "the address's scheme chooses the transport"
        );
        assert_eq!(named(&["nodes", &node], "tuning").kind, "free");
        assert_eq!(named(&["nodes", &node], ROLES).kind, "roles");
        let roles = of_table(&path(&["nodes", &node]));
        assert_eq!(roles.iter().filter(|key| key.key == ROLES).count(), 1);
        assert!(
            of_table(&[]).iter().all(|key| key.key != ROLES),
            "no shared roles"
        );
        assert_eq!(
            named(&["nodes", &node, "service"], "online").kind,
            "boolean"
        );
        let interaction = named(&["xmip_applications", "receive_locations"], "interaction");
        assert_eq!(interaction.kind, "choice");
        assert!(interaction.choices.contains(&"batch-load"));
        assert!(!interaction.required);
        let destination = named(&["xmip_applications", "subscriptions"], "destination");
        assert_eq!(destination.kind, "variant");
        assert!(destination.required);
    }

    #[test]
    fn a_bound_location_holds_a_location_and_the_node_that_takes_it() {
        let node = crate::fixture::test_cluster().node(0).name.clone();
        for table in [
            path(&["applications", "receive_locations"]),
            path(&["nodes", &node, "applications", "send_ports"]),
        ] {
            let keys: Vec<_> = of_table(&table)
                .iter()
                .map(|key| key.key.as_str())
                .collect();
            for expected in ["node", "name", "address", "settings"] {
                assert!(keys.contains(&expected), "{table:?}: {keys:?}");
            }
            let node_key = of_table(&table).into_iter().find(|key| key.key == "node");
            assert!(node_key.is_some_and(|key| key.required && key.kind == "text"));
        }
    }
}
