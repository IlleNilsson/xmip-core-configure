//! What a designer does to an Xmip Application, applied to its text
//! (ADR-0064 clause 4: a design is text, and the designer is a view of it).
//!
//! An edit is checked against the Application as it reads — a Subscription
//! it names exists, a destination is one a Subscription may route to and
//! the Application declares, a name is new — and then made to the TOML in
//! place: the lines it touches change, and the developer's comments,
//! order and layout everywhere else stay as they were. A designer never
//! writes TOML itself; it sends one of these and shows the text that comes
//! back.

use path::expression::Expression;
use route::Subscriber;
use serde::{Deserialize, Serialize};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

use crate::application::{XmipApplicationDocument, parse_application};
use crate::filter::{self, FilterPart};
use crate::routes::destination_of;

/// One act of a designer on an Application.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum ApplicationEdit {
    /// Declare a Receive Location.
    AddReceiveLocation { name: String },
    /// Declare an Xmip Process a Subscription may route to.
    AddXmipProcess { name: String },
    /// Declare a Send Port.
    AddSendPort { name: String },
    /// Add a Subscription routing everything published to `target`, a
    /// node id from [`crate::routes::Routes`]; its filter is set after.
    AddSubscription { id: String, target: String },
    /// Replace a Subscription's filter with the one these rows say, written
    /// as its canonical line.
    SetFilter {
        subscription: String,
        filter: FilterPart,
    },
    /// Route a Subscription to `target` instead.
    Connect {
        subscription: String,
        target: String,
    },
}

/// The Application in `source` with `edit` made to it.
///
/// # Errors
/// A sentence, when the Application does not read, the edit names what it
/// does not declare or declares twice, or its filter does not read.
pub fn apply(source: &str, edit: &ApplicationEdit) -> Result<String, String> {
    let document = parse_application(source)
        .map_err(|error| format!("the Application must read before it is edited: {error}"))?;
    let mut text = source
        .parse::<DocumentMut>()
        .map_err(|error| error.to_string())?;

    match edit {
        ApplicationEdit::AddReceiveLocation { name } => {
            let declared = document.receive_locations.iter().map(|e| e.name.as_str());
            add(
                &mut text,
                "receive_locations",
                "Receive Location",
                name,
                declared,
            )?;
        }
        ApplicationEdit::AddXmipProcess { name } => {
            let declared = document.xmip_processes.iter().map(|e| e.name.as_str());
            add(&mut text, "xmip_processes", "Xmip Process", name, declared)?;
        }
        ApplicationEdit::AddSendPort { name } => {
            let declared = document.send_ports.iter().map(|e| e.name.as_str());
            add(&mut text, "send_ports", "Send Port", name, declared)?;
        }
        ApplicationEdit::AddSubscription { id, target } => {
            let destination = destination(&document, target)?;
            let declared = document.subscriptions.iter().map(|s| s.id.as_str());
            let table = entry(
                &mut text,
                "subscriptions",
                "Subscription",
                "id",
                id,
                declared,
            )?;
            table["destination"] = value(inline(&destination));
            table["filter"] = value(Expression::everything().text());
        }
        ApplicationEdit::SetFilter {
            subscription,
            filter: rows,
        } => {
            let filter = filter::expression(rows)?;
            subscription_table(&mut text, &document, subscription)?["filter"] =
                value(filter.text());
        }
        ApplicationEdit::Connect {
            subscription,
            target,
        } => {
            let destination = destination(&document, target)?;
            subscription_table(&mut text, &document, subscription)?["destination"] =
                value(inline(&destination));
        }
    }

    let edited = text.to_string();
    parse_application(&edited).map_err(|error| format!("the edit would not read: {error}"))?;
    Ok(edited)
}

fn destination(document: &XmipApplicationDocument, target: &str) -> Result<Subscriber, String> {
    destination_of(document, target).ok_or_else(|| {
        format!(
            "'{target}' is not an Xmip Process, Send Port or Send Port Group the \
             Application declares"
        )
    })
}

fn inline(destination: &Subscriber) -> toml_edit::Value {
    // A destination is one key and one text; its serialization cannot fail.
    destination
        .serialize(toml_edit::ser::ValueSerializer::new())
        .expect("a destination is always TOML")
}

fn add<'a>(
    text: &mut DocumentMut,
    list: &str,
    what: &str,
    name: &str,
    declared: impl Iterator<Item = &'a str>,
) -> Result<(), String> {
    entry(text, list, what, "name", name, declared).map(|_| ())
}

/// A new table at the end of `list`, keyed `key = name`, when `name` is
/// present and not yet declared.
fn entry<'t, 'a>(
    text: &'t mut DocumentMut,
    list: &str,
    what: &str,
    key: &str,
    name: &str,
    mut declared: impl Iterator<Item = &'a str>,
) -> Result<&'t mut Table, String> {
    if name.trim().is_empty() {
        return Err(format!("a {what} requires a name"));
    }
    if declared.any(|existing| existing == name) {
        return Err(format!(
            "the Application already declares the {what} '{name}'"
        ));
    }

    let tables = text
        .entry(list)
        .or_insert_with(|| Item::ArrayOfTables(ArrayOfTables::new()))
        .as_array_of_tables_mut()
        .ok_or_else(|| format!("{list} is written inline; the designer adds to [[{list}]]"))?;

    let mut table = Table::new();
    table.decor_mut().set_prefix("\n");
    table[key] = value(name);
    tables.push(table);

    let added = tables.len() - 1;
    tables
        .get_mut(added)
        .ok_or_else(|| format!("{list} lost what was added"))
}

fn subscription_table<'t>(
    text: &'t mut DocumentMut,
    document: &XmipApplicationDocument,
    id: &str,
) -> Result<&'t mut Table, String> {
    let index = document
        .subscriptions
        .iter()
        .position(|subscription| subscription.id == id)
        .ok_or_else(|| format!("the Application has no Subscription '{id}'"))?;

    text.get_mut("subscriptions")
        .and_then(Item::as_array_of_tables_mut)
        .and_then(|tables| tables.get_mut(index))
        .ok_or_else(|| {
            "subscriptions is written inline; the designer edits [[subscriptions]]".into()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::Join;

    const ORDERS: &str = r#"# The orders integration, drawn in the designer.
[application]
name = "Orders"

[[receive_locations]]
name = "OrdersIn"

[[xmip_processes]]
name = "Approval"

[[send_ports]]
name = "Billing" # the finance system

[[subscriptions]]
id = "billing"
destination = { send-port = "Billing" }
# Written by hand, not in the canonical form: no edit but its own rewrites it.
filter = "MessageType='Order'"
"#;

    fn edit(edit: &ApplicationEdit) -> String {
        apply(ORDERS, edit).expect("applies")
    }

    #[test]
    fn adding_a_subscription_routes_everything_to_its_target_and_keeps_the_rest() {
        let edited = edit(&ApplicationEdit::AddSubscription {
            id: "approval".to_string(),
            target: "xmip-process:Approval".to_string(),
        });

        assert!(edited.starts_with(ORDERS), "{edited}");
        assert_eq!(
            &edited[ORDERS.len()..],
            "\n[[subscriptions]]\nid = \"approval\"\ndestination = { process = \"Approval\" }\n\
             filter = \"true\"\n"
        );
    }

    #[test]
    fn setting_a_filter_changes_its_line_and_nothing_else() {
        let rows = FilterPart::Group {
            join: Join::And,
            parts: vec![FilterPart::Condition {
                property: "Amount".to_string(),
                operator: ">".to_string(),
                value: "1000".to_string(),
                kind: "integer".to_string(),
            }],
        };
        let edited = edit(&ApplicationEdit::SetFilter {
            subscription: "billing".to_string(),
            filter: rows,
        });

        assert_eq!(
            edited,
            ORDERS.replace("\"MessageType='Order'\"", "\"Amount > 1000\"")
        );
    }

    #[test]
    fn connecting_changes_the_destination_only() {
        let edited = edit(&ApplicationEdit::Connect {
            subscription: "billing".to_string(),
            target: "xmip-process:Approval".to_string(),
        });

        assert_eq!(
            edited,
            ORDERS.replace("{ send-port = \"Billing\" }", "{ process = \"Approval\" }")
        );
    }

    #[test]
    fn adding_a_part_puts_its_table_after_the_last_of_its_kind() {
        let edited = edit(&ApplicationEdit::AddSendPort {
            name: "Ledger".to_string(),
        });

        let billing = "name = \"Billing\" # the finance system\n";
        let with_ledger = format!("{billing}\n[[send_ports]]\nname = \"Ledger\"\n");
        assert_eq!(edited, ORDERS.replace(billing, &with_ledger));
        assert!(
            parse_application(&edited)
                .expect("reads")
                .problems()
                .is_empty()
        );
    }

    #[test]
    fn an_edit_naming_what_is_not_there_is_refused() {
        let refused = |edit: &ApplicationEdit| apply(ORDERS, edit).expect_err("refused");

        assert_eq!(
            refused(&ApplicationEdit::Connect {
                subscription: "billing".to_string(),
                target: "send-port:Audit".to_string(),
            }),
            "'send-port:Audit' is not an Xmip Process, Send Port or Send Port Group the \
             Application declares"
        );
        assert_eq!(
            refused(&ApplicationEdit::Connect {
                subscription: "invoices".to_string(),
                target: "send-port:Billing".to_string(),
            }),
            "the Application has no Subscription 'invoices'"
        );
        assert_eq!(
            refused(&ApplicationEdit::AddReceiveLocation {
                name: "OrdersIn".to_string(),
            }),
            "the Application already declares the Receive Location 'OrdersIn'"
        );
        assert!(
            apply(
                "not toml",
                &ApplicationEdit::AddSendPort { name: "A".into() }
            )
            .is_err()
        );
    }
}
