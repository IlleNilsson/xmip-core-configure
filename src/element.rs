//! One element added to an array of unnamed tables in the cluster's file —
//! a Party's identity, say — written as an inline table, the array added
//! where it is missing (`add-element`, ADR-0064 amendment 2026-10-22).

use toml_edit::{Table, value};

use crate::view_edit::{Given, walk};

/// Append an inline table of `values` to the array at `key` of `table`.
pub(crate) fn add(table: &mut Table, key: &[String], values: &[Given]) -> Result<(), String> {
    let (last, path) = key.split_last().ok_or("an array is named by its key")?;
    let table = walk(table, path, true)?;
    let mut element = toml_edit::InlineTable::new();
    for given in values {
        let (name, within) = given
            .key
            .split_last()
            .ok_or("a value is named by its key")?;
        if !within.is_empty() {
            return Err(format!(
                "{} is not a key of one element",
                given.key.join(".")
            ));
        }
        let held = given
            .value
            .parse::<toml_edit::Value>()
            .map_err(|error| format!("{} is not a TOML value: {error}", given.value))?;
        element.insert(name, held);
    }
    let array = table
        .entry(last)
        .or_insert_with(|| value(toml_edit::Array::new()))
        .as_array_mut()
        .ok_or_else(|| format!("{} is not an array of values", key.join(".")))?;
    array.push(element);
    Ok(())
}
