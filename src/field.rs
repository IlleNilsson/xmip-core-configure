//! An entry's values as the cluster designer shows and edits them
//! ([`crate::views`], [`crate::view_edit`]): each value by its key, a
//! sub-table's by its path, written as TOML writes it.

use serde::Serialize;
use toml::{Table, Value};

/// One value, as the file writes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Field {
    /// Its key within the entry: one key, or a sub-table's path to it.
    pub key: Vec<String>,
    /// The value in TOML: `"text"`, `4`, `true`, `["a", "b"]`.
    pub value: String,
    /// `text`, `integer`, `float`, `boolean`, `date` or `array`.
    pub kind: &'static str,
}

/// Every value of `table`, a sub-table's by its path. A list of tables is
/// another kind's entries and is left to it.
#[must_use]
pub fn fields(table: &Table) -> Vec<Field> {
    let mut fields = Vec::new();
    flatten(table, &[], &mut fields);
    fields
}

fn flatten(table: &Table, prefix: &[String], fields: &mut Vec<Field>) {
    for (key, value) in table {
        let path = [prefix, std::slice::from_ref(key)].concat();
        let kind = match value {
            Value::Table(inner) => {
                flatten(inner, &path, fields);
                continue;
            }
            Value::Array(items) if items.iter().any(Value::is_table) => continue,
            Value::String(_) => "text",
            Value::Integer(_) => "integer",
            Value::Float(_) => "float",
            Value::Boolean(_) => "boolean",
            Value::Datetime(_) => "date",
            Value::Array(_) => "array",
        };
        fields.push(Field {
            key: path,
            value: value.to_string(),
            kind,
        });
    }
}

/// The keys an entry of a list is named by, the first it has: its `name`,
/// or a Subscription's `id`.
pub const NAMED_BY: [&str; 2] = ["name", "id"];

/// What an entry of a list is named by, [`NAMED_BY`].
#[must_use]
pub fn entry_name(entry: &Table) -> String {
    NAMED_BY
        .iter()
        .find_map(|key| entry.get(*key).and_then(Value::as_str))
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_flattened_by_path_and_lists_of_tables_left_out() {
        let table: Table = "name = \"drop\"\nstart = true\nnodes = [\"a\"]\n\
                            [settings]\ntopic = \"orders\"\n[[subscriptions]]\nid = \"x\"\n"
            .parse()
            .expect("TOML");
        let said: Vec<_> = fields(&table)
            .into_iter()
            .map(|field| format!("{} {} {}", field.key.join("."), field.kind, field.value))
            .collect();

        assert_eq!(
            said,
            [
                "name text \"drop\"",
                "nodes array [\"a\"]",
                "settings.topic text \"orders\"",
                "start boolean true",
            ]
        );
        assert_eq!(entry_name(&table), "drop");
    }
}
