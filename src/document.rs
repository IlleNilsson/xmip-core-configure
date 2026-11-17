//! A configuration document as TOML: read, written, and one key of it set
//! in place — once, for the runtime and every surface alike (ADR-0031,
//! amendment 2026-11-17).
//!
//! What a node reads, `cluster.xmip`, a surface's own document (`gui.toml`,
//! `cli.toml`, `powershell.toml`) and the Desktop's `preferences.toml` are
//! all read by [`read`] here, so a document reads the same wherever it is
//! read; until 2026-11-17 the .NET surfaces read theirs with a TOML package
//! of their own and the estate's PowerShell its `cluster.xmip` with
//! another. A surface crosses to these through the runtime's library
//! (`xmip_operate.h` section 27), which hands them over as JSON.
//!
//! [`set`] edits text: the one value it names changes, and every comment,
//! blank line and order elsewhere stays as written, the comment trailing
//! the old value trailing the new one.

use toml::{Table, Value};
use toml_edit::{DocumentMut, Item};

/// `text` read as TOML: its root table.
///
/// # Errors
/// The text is not TOML: [`crate::PARSE_FAILED`] and the reader's words.
pub fn read(text: &str) -> Result<Table, String> {
    text.parse::<Table>()
        .map_err(|error| format!("{}: {error}", crate::PARSE_FAILED))
}

/// `document` written as TOML: its values first, then each table and array
/// of tables under its own header.
///
/// # Errors
/// A value TOML cannot write, in the writer's words.
pub fn write(document: &Table) -> Result<String, String> {
    toml::to_string(document).map_err(|error| format!("the document does not write: {error}"))
}

/// `text` with the key `key` of the table at `table` (keys from the root;
/// none is the root) set to `value`, or removed where `value` is `None`. A
/// table missing on the way is added when a value is set, and nothing is
/// changed when one is removed.
///
/// # Errors
/// The text is not TOML, or a key on the way holds a value, not a table.
pub fn set(
    text: &str,
    table: &[String],
    key: &str,
    value: Option<&Value>,
) -> Result<String, String> {
    let mut document = text
        .parse::<DocumentMut>()
        .map_err(|error| format!("{}: {error}", crate::PARSE_FAILED))?;

    let Some(at) = walk(document.as_table_mut(), table, value.is_some())? else {
        return Ok(text.to_string());
    };

    match value {
        None => {
            at.remove(key);
        }
        Some(value) => {
            let mut new = edited(value)?;
            match at.get_mut(key) {
                Some(Item::Value(old)) => {
                    *new.decor_mut() = old.decor().clone();
                    *old = new;
                }
                _ => {
                    at.insert(key, Item::Value(new));
                }
            }
        }
    }

    Ok(document.to_string())
}

/// The table at `path` beneath `root`, each one missing on the way added
/// where `make`, implicit but the last, so no empty header is written for
/// a table that only holds tables.
fn walk<'a>(
    root: &'a mut toml_edit::Table,
    path: &[String],
    make: bool,
) -> Result<Option<&'a mut toml_edit::Table>, String> {
    let mut at = root;

    for (depth, name) in path.iter().enumerate() {
        if !at.contains_key(name) {
            if !make {
                return Ok(None);
            }
            let mut added = toml_edit::Table::new();
            added.set_implicit(depth + 1 < path.len());
            at.insert(name, Item::Table(added));
        }

        at = match at.get_mut(name) {
            Some(Item::Table(next)) => next,
            _ => return Err(format!("{} is not a table", path[..=depth].join("."))),
        };
    }

    Ok(Some(at))
}

/// `value` as the editor writes one: TOML's own text of it, read back.
fn edited(value: &Value) -> Result<toml_edit::Value, String> {
    value
        .to_string()
        .parse::<toml_edit::Value>()
        .map_err(|error| format!("the value does not write: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEPT: &str =
        "# kept\n[service]\nname = \"a\" # trails\n\n# before other\n[other]\nkey = 1\n";

    fn path(names: &[&str]) -> Vec<String> {
        names.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn a_document_reads_as_its_tables_and_text_that_is_not_toml_is_refused() {
        let read = read(KEPT).expect("TOML");

        assert_eq!(read["service"]["name"].as_str(), Some("a"));
        assert_eq!(read["other"]["key"].as_integer(), Some(1));

        let refused = super::read("[service\nname =").expect_err("not TOML");
        assert!(refused.starts_with(crate::PARSE_FAILED), "{refused}");
    }

    #[test]
    fn a_written_document_reads_back_as_it_was() {
        let document = read(KEPT).expect("TOML");

        let written = write(&document).expect("writes");

        assert_eq!(read(&written).expect("reads back"), document);
    }

    #[test]
    fn a_value_set_changes_its_line_alone_and_keeps_its_trailing_comment() {
        let quoted = Value::String("b \"quoted\"".to_string());

        let written = set(KEPT, &path(&["service"]), "name", Some(&quoted)).expect("set");

        // TOML's writer takes a literal string where the text holds quotes.
        assert_eq!(
            written,
            KEPT.replace("name = \"a\"", "name = 'b \"quoted\"'")
        );
        assert_eq!(
            read(&written).expect("TOML")["service"]["name"].as_str(),
            Some("b \"quoted\"")
        );
    }

    #[test]
    fn a_missing_table_is_added_and_a_value_removed_leaves_the_rest() {
        let added = set(
            KEPT,
            &path(&["bar"]),
            "order",
            Some(&Value::from("one,two")),
        )
        .expect("added");

        assert!(added.starts_with(KEPT), "{added}");
        assert_eq!(
            read(&added).expect("TOML")["bar"]["order"].as_str(),
            Some("one,two")
        );

        let removed = set(&added, &path(&["bar"]), "order", None).expect("removed");
        assert!(
            read(&removed).expect("TOML")["bar"]
                .as_table()
                .is_some_and(Table::is_empty)
        );

        let untouched = set(KEPT, &path(&["absent"]), "key", None).expect("nothing");
        assert_eq!(untouched, KEPT);
    }

    #[test]
    fn a_nested_table_is_added_without_an_empty_header_for_its_parent() {
        let added = set(
            "",
            &path(&["outer", "inner"]),
            "word",
            Some(&Value::from("said")),
        )
        .expect("added");

        assert_eq!(added, "[outer.inner]\nword = \"said\"\n");
    }

    #[test]
    fn a_value_on_the_way_is_refused_naming_it() {
        let refused = set(
            KEPT,
            &path(&["other", "key"]),
            "deeper",
            Some(&Value::from(true)),
        )
        .expect_err("a value is no table");

        assert!(refused.contains("other.key is not a table"), "{refused}");
    }
}
