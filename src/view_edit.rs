//! What the cluster designer does to the one `xmip.toml`, applied to its
//! text (ADR-0064 clause 4 and amendment 2026-10-03: every designer is a
//! view of its own sections of the one file).
//!
//! An edit names its table by the path [`crate::views`] gives each entry —
//! keys, and an entry of a list by its name — and is made in place: the
//! lines it touches change, and every comment, order and layout elsewhere
//! stays as the developer wrote it. A value is written as TOML writes it
//! (`"text"`, `4`, `true`). An Xmip Application held as a section is edited
//! by the Application's own edits ([`crate::edit`]).
//!
//! The edit is refused when it would leave a node that sliced and read
//! before unable to, or a section that read as an Application before unable
//! to, saying why in the reader's words: the designer never writes a file
//! the runtime would refuse where it accepted the one it was given.

use serde::{Deserialize, Serialize};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

use crate::cluster::{self, NODES};
use crate::edit::{ApplicationEdit, apply_to};
use crate::field::NAMED_BY;
use crate::section::{ApplicationSection, SECTIONS};

/// One act of the cluster designer on the file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum ClusterEdit {
    /// Write `value`, TOML, at `key` in the table at `section`; a table
    /// missing on the way is added.
    Set {
        section: Vec<String>,
        key: Vec<String>,
        value: String,
    },
    /// Remove the value at `key` in the table at `section`.
    Remove {
        section: Vec<String>,
        key: Vec<String>,
    },
    /// Add an entry named `name` to the list at `section`, its last key the
    /// list, with `values`.
    AddEntry {
        section: Vec<String>,
        name: String,
        #[serde(default)]
        values: Vec<Given>,
    },
    /// Remove the table at `section`: an entry of a list, or a node.
    RemoveEntry { section: Vec<String> },
    /// Declare a node, `[nodes.<name>]`.
    AddNode { name: String },
    /// One of the Application's own edits, made to its section.
    Application {
        application: String,
        edit: ApplicationEdit,
    },
}

/// A value given with a new entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Given {
    pub key: Vec<String>,
    pub value: String,
}

/// The cluster's file `source` with `edit` made to it.
///
/// # Errors
/// A sentence: the file does not read, the edit names what is not there or
/// adds what is, a value is not TOML, or the result would not read where
/// the file did.
pub fn apply(source: &str, edit: &ClusterEdit) -> Result<String, String> {
    let mut text = source
        .parse::<DocumentMut>()
        .map_err(|error| format!("{}: {error}", crate::PARSE_FAILED))?;
    let root = text.as_table_mut();

    match edit {
        ClusterEdit::Set {
            section,
            key,
            value,
        } => put(walk(root, section, true)?, key, value)?,
        ClusterEdit::Remove { section, key } => {
            let (last, path) = key.split_last().ok_or("a value is named by its key")?;
            let table = walk(walk(root, section, false)?, path, false)?;
            table
                .remove(last)
                .ok_or_else(|| format!("there is no {} to remove", key.join(".")))?;
        }
        ClusterEdit::AddEntry {
            section,
            name,
            values,
        } => {
            let added = add_entry(root, section, name)?;
            for given in values {
                put(added, &given.key, &given.value)?;
            }
        }
        ClusterEdit::RemoveEntry { section } => remove_entry(root, section)?,
        ClusterEdit::AddNode { name } => {
            if name.trim().is_empty() {
                return Err("a node requires a name".to_string());
            }
            let nodes = walk(root, &[NODES.to_string()], true)?;
            nodes.set_implicit(true);
            if nodes.contains_key(name) {
                return Err(format!("the cluster already declares [nodes.{name}]"));
            }
            nodes.insert(name, Item::Table(Table::new()));
        }
        ClusterEdit::Application { application, edit } => {
            let document = sections(source)
                .into_iter()
                .find(|held| held.name() == application)
                .ok_or_else(|| format!("[[{SECTIONS}]] has no entry '{application}'"))?
                .application()
                .map_err(|error| {
                    format!("the Application must read before it is edited: {error}")
                })?;
            let entry = walk(root, &[SECTIONS.to_string(), application.clone()], false)?;
            apply_to(entry, &document, edit)?;
        }
    }

    let edited = text.to_string();
    still_reads(source, &edited)?;
    Ok(edited)
}

/// The table at `section`: a key steps into a table, and into a list of
/// tables together with the next step, the entry's name. With `create`, a
/// table missing on the way is added; an entry never is.
fn walk<'t>(
    mut table: &'t mut Table,
    section: &[String],
    create: bool,
) -> Result<&'t mut Table, String> {
    let mut steps = section.iter();
    while let Some(key) = steps.next() {
        if create && !table.contains_key(key) {
            table.insert(key, Item::Table(Table::new()));
        }
        let item = table
            .get_mut(key)
            .ok_or_else(|| format!("there is no [{}]", section.join(".")))?;
        table = match item {
            Item::Table(inner) => inner,
            Item::ArrayOfTables(entries) => {
                let name = steps
                    .next()
                    .ok_or_else(|| format!("[[{key}]] is a list; name its entry"))?;
                entries
                    .iter_mut()
                    .find(|entry| named(entry) == *name)
                    .ok_or_else(|| format!("[[{key}]] has no entry '{name}'"))?
            }
            _ => {
                return Err(format!(
                    "{key} is written inline; the designer edits it as a [table]"
                ));
            }
        };
    }
    Ok(table)
}

/// Write the TOML `text` at `key` in `table`, keeping the line's comment.
fn put(table: &mut Table, key: &[String], text: &str) -> Result<(), String> {
    let (last, path) = key.split_last().ok_or("a value is named by its key")?;
    let mut given = text
        .parse::<toml_edit::Value>()
        .map_err(|error| format!("{text} is not a TOML value: {error}"))?;
    let table = walk(table, path, true)?;
    match table.get_mut(last).and_then(Item::as_value_mut) {
        Some(held) => {
            let decor = held.decor().clone();
            *given.decor_mut() = decor;
            *held = given;
        }
        None => {
            given.decor_mut().clear();
            table.insert(last, value(given));
        }
    }
    Ok(())
}

fn add_entry<'t>(
    root: &'t mut Table,
    section: &[String],
    name: &str,
) -> Result<&'t mut Table, String> {
    let (list, path) = section.split_last().ok_or("an entry is added to a list")?;
    if name.trim().is_empty() {
        return Err("an entry requires a name".to_string());
    }
    let entries = walk(root, path, true)?
        .entry(list)
        .or_insert_with(|| Item::ArrayOfTables(ArrayOfTables::new()))
        .as_array_of_tables_mut()
        .ok_or_else(|| format!("{list} is not a list of tables"))?;
    if entries.iter().any(|entry| named(entry) == name) {
        return Err(format!("[[{list}]] already has an entry '{name}'"));
    }
    let mut table = Table::new();
    table.decor_mut().set_prefix("\n");
    // A Subscription is named by its id, everything else by its name.
    let key = if list == "subscriptions" {
        "id"
    } else {
        "name"
    };
    table[key] = value(name);
    entries.push(table);
    let added = entries.len() - 1;
    entries
        .get_mut(added)
        .ok_or_else(|| format!("{list} lost what was added"))
}

fn remove_entry(root: &mut Table, section: &[String]) -> Result<(), String> {
    let (last, path) = section.split_last().ok_or("name what to remove")?;
    if let Some((list, parent)) = path.split_last()
        && let Ok(holder) = walk(root, parent, false)
        && let Some(entries) = holder.get_mut(list).and_then(Item::as_array_of_tables_mut)
    {
        let index = entries
            .iter()
            .position(|entry| named(entry) == *last)
            .ok_or_else(|| format!("[[{list}]] has no entry '{last}'"))?;
        entries.remove(index);
        if entries.is_empty() {
            holder.remove(list);
        }
        return Ok(());
    }
    walk(root, path, false)?
        .remove(last)
        .map(|_| ())
        .ok_or_else(|| format!("there is no [{}]", section.join(".")))
}

fn named(entry: &Table) -> &str {
    NAMED_BY
        .iter()
        .find_map(|key| entry.get(key).and_then(Item::as_str))
        .unwrap_or_default()
}

/// Every node that sliced and read before and is still declared still
/// does, and every section that read as an Application.
fn still_reads(before: &str, after: &str) -> Result<(), String> {
    let reads = |text: &str, node: &str| {
        cluster::slice(text, node).and_then(|slice| crate::parse_toml(&slice).map(|_| ()))
    };
    let kept = cluster::nodes(after)?;
    for node in cluster::nodes(before).unwrap_or_default() {
        if kept.contains(&node) && reads(before, &node).is_ok() {
            reads(after, &node)
                .map_err(|error| format!("the node {node} would not read: {error}"))?;
        }
    }
    let after = sections(after);
    for held in sections(before)
        .iter()
        .filter(|held| held.application().is_ok())
    {
        if let Some(edited) = after.iter().find(|edited| edited.name() == held.name()) {
            edited.application()?;
        }
    }
    Ok(())
}

/// The Xmip Applications the cluster's file holds as sections; none when
/// it does not read.
fn sections(text: &str) -> Vec<ApplicationSection> {
    text.parse::<toml::Table>()
        .ok()
        .and_then(|table| table.get(SECTIONS).cloned())
        .and_then(|sections| sections.try_into().ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::tests::written;

    fn path(steps: &[&str]) -> Vec<String> {
        steps.iter().map(ToString::to_string).collect()
    }

    fn node(text: &str, name: &str) -> crate::XmipConfigurationDocument {
        crate::parse_toml(&cluster::slice(text, name).expect("slices")).expect("reads")
    }

    #[test]
    fn setting_a_value_changes_its_line_and_keeps_its_comment() {
        let (text, _, _) = written();
        let edited = apply(
            &text,
            &ClusterEdit::Set {
                section: path(&["receive_locations", "drop"]),
                key: path(&["address"]),
                value: "\"/srv/in\"".to_string(),
            },
        )
        .expect("applies");

        assert_eq!(edited, text.replace("\"/var/xmip/in\" #", "\"/srv/in\" #"));
    }

    #[test]
    fn a_table_missing_on_the_way_is_added_and_a_value_removed() {
        let (text, _, second) = written();
        let edited = apply(
            &text,
            &ClusterEdit::Set {
                section: path(&["nodes", &second, "tuning"]),
                key: path(&["receive_idle"]),
                value: "\"1m\"".to_string(),
            },
        )
        .expect("applies");
        assert_eq!(
            node(&edited, &second).tuning.0["receive_idle"].as_str(),
            Some("1m")
        );

        let removed = apply(
            &edited,
            &ClusterEdit::Remove {
                section: path(&["tuning"]),
                key: path(&["segments"]),
            },
        )
        .expect("applies");
        assert!(!removed.contains("segments"), "{removed}");
        assert!(removed.starts_with("# The cluster, as the designer is given it."));
    }

    #[test]
    fn an_entry_is_added_with_its_values_and_removed_again() {
        let (text, first, _) = written();
        let add = |values: Vec<Given>| ClusterEdit::AddEntry {
            section: path(&["nodes", &first, "receive_locations"]),
            name: "api".to_string(),
            values,
        };
        let given = |key: &str, value: &str| Given {
            key: path(&[key]),
            value: value.to_string(),
        };

        let refused = apply(&text, &add(Vec::new())).expect_err("start is required");
        assert!(
            refused.contains("would not read") && refused.contains("start"),
            "{refused}"
        );

        let edited = apply(
            &text,
            &add(vec![
                given("start", "true"),
                given("transport", "\"xmip-core-transport-http\""),
                given("address", "\"https://in.example\""),
            ]),
        )
        .expect("applies");
        let names: Vec<_> = node(&edited, &first)
            .receive_locations
            .into_iter()
            .map(|location| location.name)
            .collect();
        assert_eq!(names, ["drop", "api"]);

        let section = path(&["nodes", &first, "receive_locations", "api"]);
        let removed = apply(&edited, &ClusterEdit::RemoveEntry { section }).expect("removes");
        assert_eq!(node(&removed, &first).receive_locations.len(), 1);
        assert!(apply(&text, &add(Vec::new())).is_err());
    }

    #[test]
    fn a_node_is_declared_once_and_removed() {
        let (text, first, _) = written();
        let absent = crate::fixture::test_cluster().absent();
        let added = apply(
            &text,
            &ClusterEdit::AddNode {
                name: absent.clone(),
            },
        )
        .expect("adds");
        assert!(cluster::nodes(&added).expect("reads").contains(&absent));
        assert!(
            apply(&added, &ClusterEdit::AddNode { name: absent })
                .expect_err("twice")
                .contains("already")
        );
        let removed = apply(
            &text,
            &ClusterEdit::RemoveEntry {
                section: path(&["nodes", &first]),
            },
        )
        .expect("removes");
        assert!(!cluster::nodes(&removed).expect("reads").contains(&first));
    }

    #[test]
    fn an_application_section_takes_the_applications_own_edits() {
        let (text, _, _) = written();
        let edit = |edit: ApplicationEdit| ClusterEdit::Application {
            application: "Orders".to_string(),
            edit,
        };
        let edited = apply(
            &text,
            &edit(ApplicationEdit::AddSendPort {
                name: "Ledger".to_string(),
            }),
        )
        .expect("applies");
        let edited = apply(
            &edited,
            &edit(ApplicationEdit::Connect {
                subscription: "billing".to_string(),
                target: "send-port:Ledger".to_string(),
            }),
        )
        .expect("applies");

        let held = &sections(&edited)[0];
        let document = held.application().expect("reads");
        assert_eq!(document.send_ports.len(), 2);
        assert_eq!(
            document.subscriptions[0].destination,
            route::Subscriber::SendPort("Ledger".to_string())
        );
        // The new Send Port stays in its section, and the rest as written.
        let at = |what: &str| edited.find(what).expect("written");
        assert!(at("name = \"Ledger\"") < at("[[applications]]"), "{edited}");
        assert!(edited.contains(&text[text.find("[[applications]]").expect("bound")..]));
        assert!(edited.starts_with("# The cluster, as the designer is given it."));
        assert!(
            apply(
                &text,
                &edit(ApplicationEdit::AddSendPort {
                    name: "Billing".into()
                })
            )
            .expect_err("declared")
            .contains("already declares")
        );
    }

    #[test]
    fn what_is_not_there_and_what_is_not_toml_are_refused() {
        let (text, _, _) = written();
        let refused = |edit: ClusterEdit| apply(&text, &edit).expect_err("refused");

        assert!(
            refused(ClusterEdit::Set {
                section: path(&["receive_locations", "drop"]),
                key: path(&["address"]),
                value: "not toml".to_string(),
            })
            .contains("not a TOML value")
        );
        assert!(
            refused(ClusterEdit::Set {
                section: path(&["receive_locations", "inbox"]),
                key: path(&["address"]),
                value: "\"x\"".to_string(),
            })
            .contains("no entry 'inbox'")
        );
        assert!(
            refused(ClusterEdit::Remove {
                section: path(&["tuning"]),
                key: path(&["threads"]),
            })
            .contains("no threads")
        );
        assert!(
            refused(ClusterEdit::Application {
                application: "Invoices".to_string(),
                edit: ApplicationEdit::AddSendPort { name: "A".into() },
            })
            .contains("no entry 'Invoices'")
        );
    }
}
