//! A Subscription's entry in its Xmip Application, as the file says it
//! (ADR-0013, amendment 2026-09-30).
//!
//! An operator who opens a Subscription sees its configuration where it is
//! kept: the `[[subscriptions]]` table of the Application that draws it,
//! with the developer's own layout and comments, read from the text rather
//! than written again from the parsed document.

use toml_edit::{DocumentMut, Item};

/// The `[[subscriptions]]` entry of `id` in the Application `source`, as
/// written; `None` when the text does not read or declares no such entry.
#[must_use]
pub fn subscription_entry(source: &str, id: &str) -> Option<String> {
    let text = source.parse::<DocumentMut>().ok()?;
    let tables = text
        .get("subscriptions")
        .and_then(Item::as_array_of_tables)?;
    let table = tables
        .iter()
        .find(|table| table.get("id").and_then(Item::as_str) == Some(id))?;
    let body = table.to_string();
    Some(format!("[[subscriptions]]\n{}", body.trim_matches('\n')))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDERS: &str = "[application]\nname = \"Orders\"\n\n\
        [[subscriptions]]\nid = \"billing\"\ndestination = { send-port = \"Billing\" }\n\
        filter = \"MessageType = 'Order'\" # the finance system's\n\n\
        [[subscriptions]]\nid = \"archive\"\ndestination = { send-port = \"Archive\" }\n\
        filter = \"true\"\n";

    #[test]
    fn the_entry_is_the_files_own_text_and_no_other_entry() {
        let billing = subscription_entry(ORDERS, "billing").expect("declared");
        assert!(billing.starts_with("[[subscriptions]]\nid = \"billing\""));
        assert!(billing.contains("# the finance system's"), "{billing}");
        assert!(!billing.contains("archive"), "{billing}");
        assert_eq!(subscription_entry(ORDERS, "shipping"), None);
        assert_eq!(subscription_entry("not = [toml", "billing"), None);
    }
}
