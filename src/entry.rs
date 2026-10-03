//! A Subscription's entry in its Xmip Application, as the file says it
//! (ADR-0013, amendment 2026-09-30).
//!
//! An operator who opens a Subscription sees its configuration where it is
//! kept: the `[[xmip_applications.subscriptions]]` table of the Application
//! section that draws it, with the file's own layout and comments, read
//! from the text rather than written again from the parsed document.

use toml_edit::{ArrayOfTables, DocumentMut, Item, Table};

use crate::section::SECTIONS;

/// The `[[xmip_applications.subscriptions]]` entry `id` of the Application
/// `application` in the configuration `source`, as written; `None` when
/// the text does not read or holds no such entry.
#[must_use]
pub fn subscription_entry(source: &str, application: &str, id: &str) -> Option<String> {
    let text = source.parse::<DocumentMut>().ok()?;
    let named =
        |table: &&Table, key: &str, name: &str| table.get(key).and_then(Item::as_str) == Some(name);
    let section = text
        .get(SECTIONS)
        .and_then(Item::as_array_of_tables)?
        .iter()
        .find(|section| named(section, "name", application))?;
    let table = section
        .get("subscriptions")
        .and_then(Item::as_array_of_tables)?
        .iter()
        .find(|table| named(table, "id", id))?;
    // Written as a document of its own, so a sub-table — a destination the
    // slice wrote as `[xmip_applications.subscriptions.destination]` — comes
    // with it, under its header.
    let mut entry = table.clone();
    entry.decor_mut().clear();
    let mut list = ArrayOfTables::new();
    list.push(entry);
    let mut holder = Table::new();
    holder.set_implicit(true);
    holder.insert("subscriptions", Item::ArrayOfTables(list));
    let mut written = DocumentMut::new();
    written.insert(SECTIONS, Item::Table(holder));
    Some(written.to_string().trim_matches('\n').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDERS: &str = "[[xmip_applications]]\nname = \"Orders\"\n\n\
        [[xmip_applications.subscriptions]]\nid = \"billing\"\n\
        destination = { send-port = \"Billing\" }\n\
        filter = \"MessageType = 'Order'\" # the finance system's\n\n\
        [[xmip_applications.subscriptions]]\nid = \"archive\"\n\
        destination = { send-port = \"Archive\" }\nfilter = \"true\"\n\n\
        [[xmip_applications]]\nname = \"Invoices\"\n\n\
        [[xmip_applications.subscriptions]]\nid = \"shipping\"\n\
        destination = { send-port = \"Shipping\" }\nfilter = \"true\"\n";

    #[test]
    fn the_entry_is_the_files_own_text_and_no_other_entry() {
        let billing = subscription_entry(ORDERS, "Orders", "billing").expect("declared");
        assert!(billing.starts_with("[[xmip_applications.subscriptions]]\nid = \"billing\""));
        assert!(billing.contains("# the finance system's"), "{billing}");
        assert!(!billing.contains("archive"), "{billing}");
        assert_eq!(subscription_entry(ORDERS, "Orders", "shipping"), None);
        assert!(subscription_entry(ORDERS, "Invoices", "shipping").is_some());
        assert_eq!(subscription_entry("not = [toml", "Orders", "billing"), None);
    }

    /// A slice writes a destination as a sub-table; the entry carries it.
    #[test]
    fn a_destination_written_as_a_sub_table_comes_with_the_entry() {
        let sliced = "[[xmip_applications]]\nname = \"Orders\"\n\n\
            [[xmip_applications.subscriptions]]\nfilter = \"true\"\nid = \"billing\"\n\n\
            [xmip_applications.subscriptions.destination]\nsend-port = \"Billing\"\n";
        let billing = subscription_entry(sliced, "Orders", "billing").expect("declared");

        assert!(
            billing.starts_with("[[xmip_applications.subscriptions]]\n"),
            "{billing}"
        );
        assert!(
            billing.contains("[xmip_applications.subscriptions.destination]\nsend-port"),
            "{billing}"
        );
    }
}
