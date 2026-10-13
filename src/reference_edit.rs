//! The edits that follow what names what ([`crate::references`]), made in
//! place in the cluster's `xmip.toml` as [`crate::view_edit`] makes every
//! edit (ADR-0064, amendment 2026-10-13: create, edit, delete, rename):
//!
//! - **rename** an entry or a node, and every name in the file that stands
//!   for it;
//! - **remove** an entry or a node still named elsewhere: refused, saying
//!   which reference holds it, unless its references go with it — the
//!   entry holding one goes when the name is what names that entry or a key
//!   it cannot be without, an array's element goes from its array, and any
//!   other key goes from its table;
//! - **refer** a reference to another name, the way a dangling one is set
//!   right;
//! - **rename a key**, its value and its place in its table kept.

use toml_edit::{Item, Key, Table, Value};

use crate::cluster::NODES;
use crate::field::NAMED_BY;
use crate::keys::of_table;
use crate::references::{Reference, references};
use crate::view_edit::{remove_entry, walk};
use crate::written::written;

/// Rename the entry or node at `section` to `name`, and every reference to
/// it.
///
/// # Errors
/// An empty name, a name its list already holds, or what is not there.
pub(crate) fn rename(
    root: &mut Table,
    source: &str,
    section: &[String],
    name: &str,
) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("a name cannot be empty".to_string());
    }
    let (old, list) = section.split_last().ok_or("name what to rename")?;
    if old == name {
        return Ok(());
    }
    for reference in held(source, section)? {
        set(root, &reference, name)?;
    }
    if list == [NODES] {
        let nodes = walk(root, list, false)?;
        if nodes.contains_key(name) {
            return Err(format!("the cluster already declares [nodes.{name}]"));
        }
        return rekey(nodes, old, name);
    }
    let (key, parent) = list.split_last().ok_or("name what to rename")?;
    let entries = walk(root, parent, false)?
        .get_mut(key)
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| format!("there is no [[{}]]", list.join(".")))?;
    if entries.iter().any(|entry| naming(entry).1 == name) {
        return Err(format!("[[{key}]] already has an entry '{name}'"));
    }
    let entry = entries
        .iter_mut()
        .find(|entry| naming(entry).1 == *old)
        .ok_or_else(|| format!("[[{key}]] has no entry '{old}'"))?;
    let (naming_key, _) = naming(entry);
    replace(entry.get_mut(naming_key).and_then(Item::as_value_mut), name)
}

/// Remove the entry or node at `section`: refused while anything names it,
/// unless `references`, which removes what names it first.
///
/// # Errors
/// Which references hold it; or what is not there.
pub(crate) fn remove(
    root: &mut Table,
    source: &str,
    section: &[String],
    references: bool,
) -> Result<(), String> {
    let holding = held(source, section)?;
    if !holding.is_empty() && !references {
        let by: Vec<String> = holding.iter().map(said).collect();
        return Err(format!(
            "{} is still named by {}: remove its references with it, or change them first",
            section.join("."),
            by.join("; ")
        ));
    }
    let tables = written(source)?;
    for reference in &holding {
        let Ok(holder) = walk(root, &reference.holder, false) else {
            continue;
        };
        let table = tables
            .iter()
            .find(|written| written.section == reference.holder)
            .map(|written| written.table.clone())
            .unwrap_or_default();
        let first = reference.key.first().cloned().unwrap_or_default();
        let required = of_table(&table)
            .iter()
            .any(|key| key.key == first && key.required);
        if NAMED_BY.contains(&first.as_str()) || required {
            let _ = remove_entry(root, &reference.holder);
        } else if let Some(index) = reference.index {
            if let Some(array) = value_at(holder, &reference.key).and_then(Value::as_array_mut) {
                array.remove(index);
            }
        } else if let [only] = reference.key.as_slice() {
            holder.remove(only);
        } else if let Some((last, path)) = reference.key.split_last()
            && let Some(inline) = value_at(holder, path).and_then(Value::as_inline_table_mut)
        {
            inline.remove(last);
        }
    }
    remove_entry(root, section)
}

/// Make the reference at `holder`'s `key` (and `index`, in an array) name
/// `name`.
///
/// # Errors
/// What is not there, or a value that is not a name.
pub(crate) fn refer(
    root: &mut Table,
    holder: &[String],
    key: &[String],
    index: Option<usize>,
    name: &str,
) -> Result<(), String> {
    let reference = Reference {
        holder: holder.to_vec(),
        key: key.to_vec(),
        index,
        name: None,
        names: "",
        target: None,
        choices: Vec::new(),
    };
    set(root, &reference, name)
}

/// Rename the key `key` of the table at `section` to `to`, its value, its
/// comment and its place kept.
///
/// # Errors
/// What is not there, or a key the table already holds.
pub(crate) fn rename_key(
    root: &mut Table,
    section: &[String],
    key: &[String],
    to: &str,
) -> Result<(), String> {
    let (last, path) = key.split_last().ok_or("name the key to rename")?;
    let table = walk(walk(root, section, false)?, path, false)?;
    if table.contains_key(to) {
        return Err(format!("[{}] already has {to}", section.join(".")));
    }
    rekey(table, last, to)
}

/// Every reference in `source` to what is at `section`, the deepest holder
/// and the last element first, so each is found before another's change.
fn held(source: &str, section: &[String]) -> Result<Vec<Reference>, String> {
    let mut holding: Vec<Reference> = references(source)?
        .into_iter()
        .filter(|reference| reference.target.as_deref() == Some(section))
        .collect();
    holding.sort_by(|a, b| {
        b.holder
            .len()
            .cmp(&a.holder.len())
            .then(b.index.cmp(&a.index))
    });
    Ok(holding)
}

/// How a person reads where a reference is.
fn said(reference: &Reference) -> String {
    format!("{} {}", reference.holder.join("."), reference.key.join("."))
}

fn set(root: &mut Table, reference: &Reference, name: &str) -> Result<(), String> {
    let holder = walk(root, &reference.holder, false)?;
    let value = value_at(holder, &reference.key);
    let value = match reference.index {
        Some(index) => value
            .and_then(Value::as_array_mut)
            .and_then(|array| array.get_mut(index)),
        None => value,
    };
    replace(value, name)
}

/// The value at `key` within `table`, through tables and inline tables.
fn value_at<'t>(table: &'t mut Table, key: &[String]) -> Option<&'t mut Value> {
    let (first, rest) = key.split_first()?;
    let mut value = match table.get_mut(first)? {
        Item::Value(value) => value,
        Item::Table(inner) => return value_at(inner, rest),
        _ => return None,
    };
    for step in rest {
        value = value.as_inline_table_mut()?.get_mut(step)?;
    }
    Some(value)
}

/// Write `name` in place of the text `value` holds, its decoration kept.
fn replace(value: Option<&mut Value>, name: &str) -> Result<(), String> {
    let value = value.ok_or("there is no such name to change")?;
    if value.as_str().is_none() {
        return Err("only a name written as text can be changed".to_string());
    }
    let decor = value.decor().clone();
    *value = Value::from(name);
    *value.decor_mut() = decor;
    Ok(())
}

/// The key naming a list's entry and the name it holds.
fn naming(entry: &Table) -> (&'static str, &str) {
    NAMED_BY
        .iter()
        .find_map(|key| {
            entry
                .get(key)
                .and_then(Item::as_str)
                .map(|name| (*key, name))
        })
        .unwrap_or(("name", ""))
}

/// Rename `old` to `new` in `table`, every key keeping its place.
fn rekey(table: &mut Table, old: &str, new: &str) -> Result<(), String> {
    if !table.contains_key(old) {
        return Err(format!("there is no {old}"));
    }
    let order: Vec<String> = table.iter().map(|(key, _)| key.to_string()).collect();
    let mut taken = Vec::new();
    for key in &order {
        if let Some(held) = table.remove_entry(key) {
            taken.push(held);
        }
    }
    for (key, item) in taken {
        let key = if key.get() == old {
            Key::new(new)
                .with_leaf_decor(key.leaf_decor().clone())
                .with_dotted_decor(key.dotted_decor().clone())
        } else {
            key
        };
        table.insert_formatted(&key, item);
    }
    Ok(())
}
