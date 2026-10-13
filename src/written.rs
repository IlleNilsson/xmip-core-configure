//! Where each table and key of a cluster's `xmip.toml` is written, in bytes
//! of its text (ADR-0064, amendment 2026-10-13: an editor goes to a
//! definition, finds its references and outlines the file from the one
//! parse).
//!
//! [`written`] reads the text with `toml_edit`, which keeps where each part
//! was, and answers every table — a `[header]`'s, a `[[list]]` entry's, or
//! one a header implies — by the path the views name it by
//! ([`crate::views::Entry::section`]: keys, and a list's entry by its name),
//! the keys its header writes, where its header and its name are, the text
//! it covers, and each key and value in it. A surface turns the bytes into
//! its own positions; what a table is, is never read from them.

use std::ops::Range;

use serde::Serialize;
use toml_edit::{Document, Item, Table, Value};

use crate::field::NAMED_BY;

/// One table, as written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Written {
    /// Its path as the views name it.
    pub section: Vec<String>,
    /// Its keys as its header writes them, a list's entries sharing it.
    pub table: Vec<String>,
    /// Its header, `[a.b]` or `[[a.b]]`; none where no header opens it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<[usize; 2]>,
    /// What names it: a list entry's `name` or `id` value, or the key a
    /// table is held under.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<[usize; 2]>,
    /// The text it covers: from its header to the next one, and a table no
    /// header opens from its first table's header to its last one's end.
    pub extent: [usize; 2],
    pub keys: Vec<WrittenKey>,
}

/// One key and its value, as written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WrittenKey {
    /// Its path within the table: one key, a dotted key's or an inline
    /// table's keys.
    pub key: Vec<String>,
    /// Its place in an array, where it is an element of one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    /// The key's bytes; an element's own.
    pub at: [usize; 2],
    /// The value's bytes, quotes and all.
    pub value: [usize; 2],
}

/// Every table of `text`, the top first and then in the order written.
///
/// # Errors
/// The reader's words when the text is not TOML.
pub fn written(text: &str) -> Result<Vec<Written>, String> {
    let document =
        Document::parse(text).map_err(|error| format!("{}: {error}", crate::PARSE_FAILED))?;
    let mut found = vec![Written {
        section: Vec::new(),
        table: Vec::new(),
        header: None,
        name: None,
        extent: [0, text.len()],
        keys: Vec::new(),
    }];
    walk(document.as_table(), &Place::default(), 0, &mut found);
    extend(&mut found, text.len());
    Ok(found)
}

/// Where a walk is: the table's section and header path, and the dotted
/// keys it is beneath within the table.
#[derive(Clone, Default)]
struct Place {
    section: Vec<String>,
    table: Vec<String>,
    dotted: Vec<String>,
}

fn walk(table: &Table, place: &Place, current: usize, found: &mut Vec<Written>) {
    for (name, item) in table {
        let key_span = table.key(name).and_then(toml_edit::Key::span);
        match item {
            Item::Value(value) => {
                let key = [place.dotted.clone(), vec![name.to_string()]].concat();
                values(&key, key_span, value, &mut found[current].keys);
            }
            Item::Table(child) if child.is_dotted() => {
                let mut inner = place.clone();
                inner.dotted.push(name.to_string());
                walk(child, &inner, current, found);
            }
            Item::Table(child) => {
                let inner = Place {
                    section: [place.section.clone(), vec![name.to_string()]].concat(),
                    table: [place.table.clone(), vec![name.to_string()]].concat(),
                    dotted: Vec::new(),
                };
                found.push(opened(
                    &inner,
                    child.span().filter(|_| !child.is_implicit()),
                    key_span,
                ));
                let at = found.len() - 1;
                walk(child, &inner, at, found);
            }
            Item::ArrayOfTables(entries) => {
                for entry in entries {
                    let named = NAMED_BY.iter().find_map(|key| {
                        let value = entry.get(key)?.as_value()?;
                        Some((value.as_str()?.to_string(), value.span()))
                    });
                    let (entry_name, name_span) = named.unwrap_or_default();
                    let inner = Place {
                        section: [place.section.clone(), vec![name.to_string(), entry_name]]
                            .concat(),
                        table: [place.table.clone(), vec![name.to_string()]].concat(),
                        dotted: Vec::new(),
                    };
                    found.push(opened(&inner, entry.span(), name_span));
                    let at = found.len() - 1;
                    walk(entry, &inner, at, found);
                }
            }
            Item::None => {}
        }
    }
}

fn opened(place: &Place, header: Option<Range<usize>>, name: Option<Range<usize>>) -> Written {
    Written {
        section: place.section.clone(),
        table: place.table.clone(),
        header: header.map(bytes),
        name: name.map(bytes),
        extent: [0, 0],
        keys: Vec::new(),
    }
}

/// A value's key and every key and element within it.
fn values(key: &[String], at: Option<Range<usize>>, value: &Value, keys: &mut Vec<WrittenKey>) {
    let value_span = value.span().map(bytes).unwrap_or_default();
    keys.push(WrittenKey {
        key: key.to_vec(),
        index: None,
        at: at.map(bytes).unwrap_or_default(),
        value: value_span,
    });
    match value {
        Value::InlineTable(inline) => {
            for (name, inner) in inline {
                let span = inline.key(name).and_then(toml_edit::Key::span);
                let path = [key.to_vec(), vec![name.to_string()]].concat();
                values(&path, span, inner, keys);
            }
        }
        Value::Array(array) => {
            for (index, element) in array.iter().enumerate() {
                let span = element.span().map(bytes).unwrap_or_default();
                keys.push(WrittenKey {
                    key: key.to_vec(),
                    index: Some(index),
                    at: span,
                    value: span,
                });
            }
        }
        _ => {}
    }
}

fn bytes(range: Range<usize>) -> [usize; 2] {
    [range.start, range.end]
}

/// Each table's extent: from its header to the next header; the top's to
/// the first header; a table no header opens, over the tables beneath it.
fn extend(found: &mut [Written], end: usize) {
    let mut starts: Vec<usize> = found
        .iter()
        .filter_map(|written| written.header.map(|[start, _]| start))
        .collect();
    starts.sort_unstable();
    let next = |after: usize| starts.iter().copied().find(|start| *start > after);
    for written in found.iter_mut() {
        written.extent = match written.header {
            Some([start, _]) => [start, next(start).unwrap_or(end)],
            None if written.section.is_empty() => [0, starts.first().copied().unwrap_or(end)],
            None => [end, 0],
        };
    }
    for at in (0..found.len()).rev() {
        if found[at].header.is_some() || found[at].section.is_empty() {
            continue;
        }
        let section = found[at].section.clone();
        let [mut start, mut stop] = found[at].extent;
        for inner in found.iter().filter(|w| w.section.starts_with(&section)) {
            if inner.section.len() > section.len() && inner.extent[0] < inner.extent[1] {
                start = start.min(inner.extent[0]);
                stop = stop.max(inner.extent[1]);
            }
        }
        found[at].extent = if start < stop { [start, stop] } else { [0, 0] };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::tests::written as cluster;

    fn of<'w>(found: &'w [Written], section: &[&str]) -> &'w Written {
        found
            .iter()
            .find(|written| written.section == section)
            .unwrap_or_else(|| panic!("no {section:?}"))
    }

    #[test]
    fn every_table_is_found_by_its_section_with_its_header_and_name() {
        let (text, first, _) = cluster();
        let found = written(&text).expect("reads");
        let at = |range: [usize; 2]| &text[range[0]..range[1]];

        let port = of(
            &found,
            &["xmip_applications", "Orders", "send_ports", "Billing"],
        );
        assert_eq!(
            at(port.header.expect("a header")),
            "[[xmip_applications.send_ports]]"
        );
        assert_eq!(at(port.name.expect("a name")), "\"Billing\"");
        assert_eq!(port.table, ["xmip_applications", "send_ports"]);
        assert!(at(port.extent).starts_with("[[xmip_applications.send_ports]]\nname"));

        let node = of(&found, &["nodes", &first]);
        assert_eq!(at(node.name.expect("its key")), first);
        let tuning = of(&found, &["nodes", &first, "tuning"]);
        assert!(
            node.extent[0] <= tuning.extent[0] && tuning.extent[1] <= node.extent[1],
            "{:?} {:?} {:?}",
            node,
            tuning,
            found
        );
        assert!(at(of(&found, &[]).extent).starts_with("# The cluster"));
    }

    #[test]
    fn a_key_an_inline_tables_keys_and_an_arrays_elements_are_each_found() {
        let (text, _, _) = cluster();
        let text = text.replace(
            "name = \"Billing\"",
            "name = \"Billing\"\nsend_locations = [\"a\", \"b\"]",
        );
        let found = written(&text).expect("reads");
        let at = |range: [usize; 2]| &text[range[0]..range[1]];

        let subscription = of(
            &found,
            &["xmip_applications", "Orders", "subscriptions", "billing"],
        );
        let port = subscription
            .keys
            .iter()
            .find(|key| key.key == ["destination", "send-port"])
            .expect("the destination's case");
        assert_eq!(at(port.at), "send-port");
        assert_eq!(at(port.value), "\"Billing\"");

        let billing = of(
            &found,
            &["xmip_applications", "Orders", "send_ports", "Billing"],
        );
        let second = billing
            .keys
            .iter()
            .find(|key| key.index == Some(1))
            .expect("the second element");
        assert_eq!(at(second.value), "\"b\"");
        let address = of(&found, &["receive_locations", "drop"])
            .keys
            .iter()
            .find(|k| k.key == ["address"]);
        assert_eq!(at(address.expect("address").value), "\"/var/xmip/in\"");
    }
}
