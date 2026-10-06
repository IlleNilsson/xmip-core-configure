//! The Xmip Application (ADR-0064): an integration as a developer designs
//! it, drawn once and bound by every node that runs it. It is a section of
//! the cluster's one `xmip.toml`, `[[xmip_applications]]` ([`crate::section`];
//! ADR-0064 and ADR-0031, amendments 2026-10-03), and [`parse_application`]
//! is its one reading.
//!
//! It holds the design and nothing of an environment: its Receive Ports and
//! the Receive Locations at them, its Work Processes and Send Ports by name
//! with the settings the runtime takes from each ([`crate::port`]), the Send
//! Port Groups that gather Send Ports, and its Subscriptions — each
//! `route`'s own [`Subscription`], a filter — one line of Xmip's expression
//! language, compiled as the Application is read (ADR-0066) — and the
//! destination it routes to. Addresses, credentials and which node takes
//! what are the node's, in its binding ([`crate::binding`]).
//!
//! ```toml
//! [[xmip_applications]]
//! name = "Orders"
//!
//! [[xmip_applications.receive_ports]]
//! name = "Orders"
//!
//! [[xmip_applications.receive_locations]]
//! name = "OrdersIn"
//! receive_port = "Orders"
//! interaction = "data-transfer"
//! depth = "light"
//!
//! [[xmip_applications.send_ports]]
//! name = "Billing"
//!
//! [[xmip_applications.subscriptions]]
//! id = "billing"
//! destination = { send-port = "Billing" }
//! filter = "MessageType = 'Order' and not Amount > 1000"
//! ```

use std::collections::BTreeSet;

use route::{Subscriber, Subscription};
use serde::{Deserialize, Serialize};

use crate::port::{self, DesignedReceiveLocation, DesignedSendPort};

/// One Xmip Application, as its section of the cluster's TOML says it
/// (ADR-0031). A key the section does not define is refused, so a
/// misspelled list is a problem the developer sees rather than a design
/// that silently lost a part.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XmipApplication {
    /// What the Application is called, the developer's name; what a binding
    /// names it by.
    pub name: String,
    /// The Receive Ports its Receive Locations belong to: each keeps
    /// Message creation and Publication (`runtime-model.md` section 6).
    #[serde(default)]
    pub receive_ports: Vec<DesignedElement>,
    /// Where Messages enter, each at its Receive Port; the node's binding
    /// gives each its transport, address and node.
    #[serde(default)]
    pub receive_locations: Vec<DesignedReceiveLocation>,
    /// The Work Processes a Subscription may route to. A name today; the
    /// flow is the process designer's, after the vocabulary (ADR-0064 clause 1).
    #[serde(default)]
    pub work_processes: Vec<DesignedElement>,
    /// Where Messages leave, each with its policy; the binding gives each
    /// its Location.
    #[serde(default)]
    pub send_ports: Vec<DesignedSendPort>,
    /// Send Ports a Subscription reaches together.
    #[serde(default)]
    pub send_port_groups: Vec<SendPortGroup>,
    /// What each published Message is offered to, and where a match goes.
    #[serde(default)]
    pub subscriptions: Vec<Subscription>,
}

/// A Receive Port or a Work Process as designed: a name, and nothing an
/// environment decides.
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

impl XmipApplication {
    /// Whether this Application declares what `destination` names.
    #[must_use]
    pub fn declares(&self, destination: &Subscriber) -> bool {
        let name = destination.name();
        match destination {
            Subscriber::WorkProcess(_) => self.work_processes.iter().any(|p| p.name == name),
            Subscriber::SendPort(_) => self.declares_send_port(name),
            Subscriber::SendGroup(_) => self.send_port_groups.iter().any(|g| g.name == name),
        }
    }

    fn declares_send_port(&self, name: &str) -> bool {
        self.send_ports.iter().any(|port| port.name == name)
    }

    /// Everything that makes this design unusable, one sentence each; empty
    /// when it is sound.
    #[must_use]
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let application = &self.name;

        if application.trim().is_empty() {
            problems.push("an Xmip Application requires a name".to_string());
        }

        let names = |elements: &[DesignedElement]| -> Vec<String> {
            elements.iter().map(|e| e.name.clone()).collect()
        };
        for (what, declared) in [
            ("Receive Port", names(&self.receive_ports)),
            (
                "Receive Location",
                self.receive_locations
                    .iter()
                    .map(|l| l.name.clone())
                    .collect(),
            ),
            ("Work Process", names(&self.work_processes)),
            (
                "Send Port",
                self.send_ports.iter().map(|p| p.name.clone()).collect(),
            ),
            (
                "Send Port Group",
                self.send_port_groups
                    .iter()
                    .map(|g| g.name.clone())
                    .collect(),
            ),
            (
                "Subscription",
                self.subscriptions.iter().map(|s| s.id.clone()).collect(),
            ),
        ] {
            once_each(application, what, &declared, &mut problems);
        }

        port::receive_problems(
            application,
            &self.receive_ports,
            &self.receive_locations,
            &mut problems,
        );
        port::send_problems(application, &self.send_ports, &mut problems);

        for group in &self.send_port_groups {
            for member in &group.send_ports {
                if !self.declares_send_port(member) {
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

/// Read an Xmip Application from its section's text: `name` and its lists
/// ([`crate::ApplicationSection::text`]). The error is the TOML reader's
/// own words, line and column included.
///
/// # Errors
/// When the text is not TOML, lacks what the Application requires or holds
/// a key it does not define.
pub fn parse_application(source: &str) -> Result<XmipApplication, String> {
    toml::from_str(source).map_err(|error| error.to_string())
}

/// How a problem names a destination: `the Send Port 'Billing'`.
#[must_use]
pub fn destination_words(destination: &Subscriber) -> String {
    let what = match destination {
        Subscriber::WorkProcess(_) => "Work Process",
        Subscriber::SendPort(_) => "Send Port",
        Subscriber::SendGroup(_) => "Send Port Group",
    };
    format!("the {what} '{}'", destination.name())
}

/// Each name present, and present once.
fn once_each(application: &str, what: &str, names: &[String], problems: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    for name in names {
        if name.trim().is_empty() {
            problems.push(format!("a {what} of '{application}' requires a name"));
        } else if !seen.insert(name.as_str()) {
            problems.push(format!(
                "'{application}' declares the {what} '{name}' twice"
            ));
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The example `doc/application.md` opens with, as its section's text.
    pub(crate) const ORDERS: &str = r#"name = "Orders"

[[receive_ports]]
name = "Orders"

[[receive_locations]]
name = "OrdersIn"
receive_port = "Orders"
interaction = "data-transfer"
depth = "light"

[[work_processes]]
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
destination = { work-process = "Approval" }
filter = "MessageType = 'Order' and Amount > 1000"
"#;

    fn problems(source: &str) -> Vec<String> {
        parse_application(source).expect("reads").problems()
    }

    #[test]
    fn an_application_parses_with_its_subscriptions_as_route_reads_them() {
        let document = parse_application(ORDERS).expect("parses");

        assert_eq!(document.name, "Orders");
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
    }

    #[test]
    fn a_subscription_routing_to_a_missing_target_is_refused() {
        let source = ORDERS.replace(
            "{ work-process = \"Approval\" }",
            "{ send-port = \"Audit\" }",
        );

        assert_eq!(
            problems(&source),
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
        let problems = problems(&source);

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
            let refused = parse_application(&source).expect_err("refused");

            assert!(refused.contains(said), "{refused}");
        }
    }

    #[test]
    fn a_misspelled_list_is_refused_not_ignored() {
        let source = ORDERS.replace("[[send_ports]]", "[[send_port]]");
        let refused = parse_application(&source).expect_err("refused");

        assert!(refused.contains("send_port"), "{refused}");
    }
}
