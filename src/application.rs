//! The Xmip Application document (ADR-0064): an integration as a developer
//! designs it, drawn once and bound by every node that runs it.
//!
//! It holds the design and nothing of an environment: its Receive Locations,
//! Xmip Processes and Send Ports by name, the Send Port Groups that gather
//! Send Ports, and its Subscriptions — each `route`'s own
//! [`Subscription`], a filter — one line of Xmip's expression language,
//! compiled as the document is read (ADR-0066) — and the destination it
//! routes to. Addresses, credentials and which node
//! takes what are the node's, in its binding ([`crate::binding`]).
//!
//! ```toml
//! [application]
//! name = "Orders"
//!
//! [[receive_locations]]
//! name = "OrdersIn"
//!
//! [[send_ports]]
//! name = "Billing"
//!
//! [[subscriptions]]
//! id = "billing"
//! destination = { send-port = "Billing" }
//! filter = "MessageType = 'Order' and not Amount > 1000"
//! ```

use std::collections::BTreeSet;

use route::{Subscriber, Subscription};
use serde::{Deserialize, Serialize};

use crate::PARSE_FAILED;

/// One Xmip Application, as the TOML in the repository says it (ADR-0031).
/// A key the document does not define is refused, so a misspelled list is a
/// problem the developer sees rather than a design that silently lost a part.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XmipApplicationDocument {
    pub application: ApplicationHeader,
    /// Where Messages enter, by name; the node's binding gives each its
    /// transport, address and node.
    #[serde(default)]
    pub receive_locations: Vec<DesignedElement>,
    /// The Xmip Processes a Subscription may route to. A name today; the
    /// flow is the process designer's, after the vocabulary (ADR-0064 clause 1).
    #[serde(default)]
    pub xmip_processes: Vec<DesignedElement>,
    /// Where Messages leave, by name; the binding gives each its Location.
    #[serde(default)]
    pub send_ports: Vec<DesignedElement>,
    /// Send Ports a Subscription reaches together.
    #[serde(default)]
    pub send_port_groups: Vec<SendPortGroup>,
    /// What each published Message is offered to, and where a match goes.
    #[serde(default)]
    pub subscriptions: Vec<Subscription>,
}

/// `[application]`: what the Application is called, the developer's name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationHeader {
    pub name: String,
}

/// A Receive Location, an Xmip Process or a Send Port as designed: a name,
/// and nothing an environment decides.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignedElement {
    pub name: String,
}

/// Send Ports a Subscription reaches together: a Send Port Group.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendPortGroup {
    pub name: String,
    #[serde(default)]
    pub send_ports: Vec<String>,
}

impl XmipApplicationDocument {
    /// Whether this Application declares what `destination` names.
    #[must_use]
    pub fn declares(&self, destination: &Subscriber) -> bool {
        let name = destination.name();
        match destination {
            Subscriber::Process(_) => named(&self.xmip_processes, name),
            Subscriber::SendPort(_) => named(&self.send_ports, name),
            Subscriber::SendGroup(_) => self.send_port_groups.iter().any(|g| g.name == name),
        }
    }

    /// Everything that makes this design unusable, one sentence each; empty
    /// when it is sound.
    #[must_use]
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let application = &self.application.name;

        if application.trim().is_empty() {
            problems.push("an Xmip Application requires a name".to_string());
        }

        let groups = self
            .send_port_groups
            .iter()
            .map(|group| group.name.as_str());
        let subscriptions = self.subscriptions.iter().map(|s| s.id.as_str());
        for (what, names) in [
            ("Receive Location", names(&self.receive_locations)),
            ("Xmip Process", names(&self.xmip_processes)),
            ("Send Port", names(&self.send_ports)),
            ("Send Port Group", groups.collect()),
            ("Subscription", subscriptions.collect()),
        ] {
            once_each(application, what, &names, &mut problems);
        }

        for group in &self.send_port_groups {
            for member in &group.send_ports {
                if !named(&self.send_ports, member) {
                    problems.push(format!(
                        "Send Port Group '{}' holds the Send Port '{member}', which the \
                         Application does not declare",
                        group.name
                    ));
                }
            }
        }

        for subscription in &self.subscriptions {
            if !self.declares(&subscription.destination) {
                problems.push(format!(
                    "Subscription '{}' routes to {}, which the Application does not declare",
                    subscription.id,
                    destination_words(&subscription.destination)
                ));
            }
        }

        problems
    }
}

/// Read an Xmip Application. The error is the TOML reader's own words, line
/// and column included.
///
/// # Errors
/// When the text is not TOML, lacks what the document requires or holds a
/// key it does not define.
pub fn parse_application(source: &str) -> Result<XmipApplicationDocument, String> {
    toml::from_str(source).map_err(|error| error.to_string())
}

/// What is wrong with the Application in `source`, one problem per entry:
/// the reading's failure alone when it does not parse, else
/// [`XmipApplicationDocument::problems`]. Empty when it is sound.
#[must_use]
pub fn application_problems(source: &str) -> Vec<String> {
    match parse_application(source) {
        Ok(document) => document.problems(),
        Err(error) => vec![format!("{PARSE_FAILED}: {error}")],
    }
}

/// How a problem names a destination: `the Send Port 'Billing'`.
#[must_use]
pub fn destination_words(destination: &Subscriber) -> String {
    let what = match destination {
        Subscriber::Process(_) => "Xmip Process",
        Subscriber::SendPort(_) => "Send Port",
        Subscriber::SendGroup(_) => "Send Port Group",
    };
    format!("the {what} '{}'", destination.name())
}

fn names(elements: &[DesignedElement]) -> Vec<&str> {
    elements
        .iter()
        .map(|element| element.name.as_str())
        .collect()
}

fn named(elements: &[DesignedElement], name: &str) -> bool {
    elements.iter().any(|element| element.name == name)
}

/// Each name present, and present once.
fn once_each(application: &str, what: &str, names: &[&str], problems: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    for name in names {
        if name.trim().is_empty() {
            problems.push(format!("a {what} of '{application}' requires a name"));
        } else if !seen.insert(*name) {
            problems.push(format!(
                "'{application}' declares the {what} '{name}' twice"
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The example `doc/application.md` opens with.
    pub(crate) const ORDERS: &str = r#"[application]
name = "Orders"

[[receive_locations]]
name = "OrdersIn"

[[xmip_processes]]
name = "Approval"

[[send_ports]]
name = "Billing"

[[send_ports]]
name = "Ledger"

[[send_port_groups]]
name = "Books"
send_ports = ["Billing", "Ledger"]

[[subscriptions]]
id = "billing"
destination = { send-port = "Billing" }
filter = "MessageType = 'Order' and not Amount > 1000"

[[subscriptions]]
id = "approval"
destination = { process = "Approval" }
filter = "MessageType = 'Order' and Amount > 1000"
"#;

    #[test]
    fn an_application_parses_with_its_subscriptions_as_route_reads_them() {
        let document = parse_application(ORDERS).expect("parses");

        assert_eq!(document.application.name, "Orders");
        assert_eq!(document.receive_locations[0].name, "OrdersIn");
        assert_eq!(document.subscriptions.len(), 2);
        assert_eq!(
            document.subscriptions[0].destination,
            Subscriber::SendPort("Billing".to_string())
        );
        assert_eq!(
            document.subscriptions[1].filter.text(),
            "MessageType = 'Order' and Amount > 1000"
        );
        assert_eq!(
            document.subscriptions[1].filter.names(),
            ["Amount", "MessageType"]
        );
        assert!(document.problems().is_empty(), "{:?}", document.problems());
        assert!(application_problems(ORDERS).is_empty());
    }

    #[test]
    fn a_subscription_routing_to_a_missing_target_is_refused() {
        let source = ORDERS.replace("{ process = \"Approval\" }", "{ send-port = \"Audit\" }");

        assert_eq!(
            application_problems(&source),
            [
                "Subscription 'approval' routes to the Send Port 'Audit', which the Application \
              does not declare"
            ]
        );
    }

    #[test]
    fn a_group_member_that_is_not_declared_and_a_name_twice_are_refused() {
        let source = ORDERS
            .replace("[\"Billing\", \"Ledger\"]", "[\"Billing\", \"Archive\"]")
            .replace("name = \"Ledger\"", "name = \"Billing\"");
        let problems = application_problems(&source);

        assert!(
            problems.contains(&"'Orders' declares the Send Port 'Billing' twice".to_string()),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|problem| problem.contains("'Archive'")),
            "{problems:?}"
        );
    }

    #[test]
    fn a_filter_that_does_not_compile_refuses_the_application_as_it_loads() {
        for (broken, said) in [
            ("Amount > 'x' + 1", "arithmetic"),
            ("Amount == 1000", "equality is '='"),
            ("Amount > 12.5", "decimal"),
        ] {
            let source = ORDERS.replace("Amount > 1000\"", &format!("{broken}\""));
            let problems = application_problems(&source);

            assert_eq!(problems.len(), 1, "{problems:?}");
            assert!(problems[0].starts_with(PARSE_FAILED), "{}", problems[0]);
            assert!(problems[0].contains(said), "{}", problems[0]);
        }
    }

    #[test]
    fn a_misspelled_list_is_refused_not_ignored() {
        let source = ORDERS.replace("[[send_ports]]", "[[send_port]]");
        let problems = application_problems(&source);

        assert_eq!(problems.len(), 1);
        assert!(problems[0].starts_with(PARSE_FAILED), "{}", problems[0]);
        assert!(problems[0].contains("send_port"), "{}", problems[0]);
    }
}
