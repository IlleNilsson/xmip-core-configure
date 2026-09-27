//! A node's binding of an Xmip Application (ADR-0064): that the node runs
//! it, and the environment's side of what it declares.
//!
//! The Application holds the design, drawn once; each node's configuration
//! names the Applications it runs in `[[applications]]` and gives every
//! Receive Location and Send Port it binds a transport, an address, a
//! reference to its credentials and the node that takes it, as the
//! bindings of the platforms Xmip replaces do. The same route is never
//! written again per node.
//!
//! ```toml
//! [[applications]]
//! name = "Orders"
//! document = "orders.application.toml"
//!
//! [[applications.receive_locations]]
//! name = "OrdersIn"
//! node = "alpha"
//! start = true
//! transport = "xmip-core-transport-file"
//! address = "/var/xmip/in/orders"
//! ```
//!
//! [`binding_problems`] checks what a binding says on its own; [`bind`]
//! joins it to the Applications it names and takes what one node runs.

use std::collections::BTreeSet;

use route::Subscription;
use serde::{Deserialize, Serialize};

use crate::application::XmipApplicationDocument;
use crate::{ConfiguredLocation, XmipConfigurationDocument};

/// `[[applications]]`: one Xmip Application this node runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationBinding {
    /// The Application's own name, `[application] name` in its document.
    pub name: String,
    /// Where its document is: a path relative to this configuration's file.
    pub document: String,
    /// The Receive Locations it declares that this environment runs.
    #[serde(default)]
    pub receive_locations: Vec<BoundLocation>,
    /// The Send Ports it declares, each given the Send Location it leaves by.
    #[serde(default)]
    pub send_ports: Vec<BoundLocation>,
}

/// A Receive Location or a Send Port an Application declares, given its
/// environment: the node that takes it, and the Location's transport,
/// address, start and credentials reference, in the node's own shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundLocation {
    /// The node that takes it, a `service.node_name`.
    pub node: String,
    #[serde(flatten)]
    pub location: ConfiguredLocation,
}

/// What one node runs of the Applications it binds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bound {
    /// The bound Receive Locations this node takes.
    pub receive_locations: Vec<ConfiguredLocation>,
    /// The bound Send Ports this node takes, as the Send Locations they are.
    pub send_locations: Vec<ConfiguredLocation>,
    /// Every Subscription of every bound Application.
    pub subscriptions: Vec<Subscription>,
}

/// What the bindings in `document` say wrong on their own, before any
/// Application is read: a name, a document, and every bound Location's
/// node, name and transport present, and no Location bound twice.
#[must_use]
pub fn binding_problems(document: &XmipConfigurationDocument) -> Vec<String> {
    let mut problems = Vec::new();
    let mut bound = BTreeSet::new();

    for binding in &document.applications {
        let application = &binding.name;
        if application.trim().is_empty() {
            problems.push("a bound Xmip Application requires a name".to_string());
        } else if !bound.insert(application.as_str()) {
            problems.push(format!(
                "the Xmip Application '{application}' is bound twice"
            ));
        }
        if binding.document.trim().is_empty() {
            problems.push(format!(
                "the binding of '{application}' requires the document it reads"
            ));
        }

        for (what, locations) in [
            ("Receive Location", &binding.receive_locations),
            ("Send Port", &binding.send_ports),
        ] {
            let mut seen = BTreeSet::new();
            for bound in locations {
                let name = &bound.location.name;
                if name.trim().is_empty() {
                    problems.push(format!("a bound {what} of '{application}' requires a name"));
                } else if !seen.insert(name.as_str()) {
                    problems.push(format!("'{application}' binds the {what} '{name}' twice"));
                }
                if bound.node.trim().is_empty() {
                    problems.push(format!(
                        "the {what} '{name}' of '{application}' requires the node that takes it"
                    ));
                }
                if bound.location.transport.trim().is_empty() {
                    problems.push(format!(
                        "the {what} '{name}' of '{application}' requires a transport"
                    ));
                }
            }
        }
    }

    problems
}

/// Join the node's bindings to the Applications it was given and take what
/// this node runs: the bound Locations whose `node` is this node's name,
/// and every Subscription of every bound Application.
///
/// # Errors
/// Every problem found, one sentence each: a binding's own
/// ([`binding_problems`]), an Application the node binds and was not given,
/// a bound Application's own problems, a Location it does not declare, and
/// a Subscription or Location name two of them share on this node.
pub fn bind(
    document: &XmipConfigurationDocument,
    applications: &[XmipApplicationDocument],
) -> Result<Bound, Vec<String>> {
    let mut problems = binding_problems(document);
    let node = &document.service.node_name;
    let mut bound = Bound::default();

    for binding in &document.applications {
        let name = &binding.name;
        let Some(application) = applications.iter().find(|a| &a.application.name == name) else {
            problems.push(format!(
                "the node binds the Xmip Application '{name}', which it was not given"
            ));
            continue;
        };

        for problem in application.problems() {
            problems.push(format!("Xmip Application '{name}': {problem}"));
        }

        for (what, locations, declared, into) in [
            (
                "Receive Location",
                &binding.receive_locations,
                &application.receive_locations,
                &mut bound.receive_locations,
            ),
            (
                "Send Port",
                &binding.send_ports,
                &application.send_ports,
                &mut bound.send_locations,
            ),
        ] {
            for location in locations {
                let wanted = &location.location.name;
                if declared.iter().any(|element| &element.name == wanted) {
                    if &location.node == node {
                        into.push(location.location.clone());
                    }
                } else {
                    problems.push(format!(
                        "the node binds the {what} '{wanted}' of '{name}', which '{name}' \
                         does not declare"
                    ));
                }
            }
        }

        bound
            .subscriptions
            .extend(application.subscriptions.iter().cloned());
    }

    once_on_the_node(document, &bound, &mut problems);

    if problems.is_empty() {
        Ok(bound)
    } else {
        Err(problems)
    }
}

/// A Subscription id, and a Location name at each stage, is one thing on a
/// node: two bound Applications, or an Application and the node itself,
/// may not both use it there.
fn once_on_the_node(
    document: &XmipConfigurationDocument,
    bound: &Bound,
    problems: &mut Vec<String>,
) {
    let subscriptions = bound.subscriptions.iter().map(|s| s.id.as_str());
    let receive = document
        .receive_locations
        .iter()
        .chain(&bound.receive_locations);
    let send = document.send_locations.iter().chain(&bound.send_locations);

    for (what, names) in [
        ("Subscription", subscriptions.collect::<Vec<_>>()),
        (
            "Receive Location",
            receive.map(|l| l.name.as_str()).collect(),
        ),
        ("Send Location", send.map(|l| l.name.as_str()).collect()),
    ] {
        let mut seen = BTreeSet::new();
        for name in names {
            if !seen.insert(name) {
                problems.push(format!("the {what} '{name}' is on this node twice"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_application, parse_toml};

    const ORDERS: &str = r#"[application]
name = "Orders"

[[receive_locations]]
name = "OrdersIn"

[[send_ports]]
name = "Billing"

[[subscriptions]]
id = "billing"
destination = { send-port = "Billing" }
filter = "MessageType = 'Order'"
"#;

    /// Node alpha of the example in `doc/node-configuration.md`: it takes the
    /// Receive Location, and beta takes the Send Port.
    const ALPHA: &str = r#"[service]
name = "xmip-alpha"
cluster_name = "orders"
node_name = "alpha"

[[applications]]
name = "Orders"
document = "orders.application.toml"

[[applications.receive_locations]]
name = "OrdersIn"
node = "alpha"
start = true
transport = "xmip-core-transport-file"
address = "/var/xmip/in/orders"
credentials = "orders-in"

[[applications.send_ports]]
name = "Billing"
node = "beta"
start = true
transport = "xmip-core-transport-http"
address = "https://billing.example/orders"
"#;

    fn orders() -> XmipApplicationDocument {
        parse_application(ORDERS).expect("parses")
    }

    #[test]
    fn a_binding_binds_what_this_node_takes_and_every_subscription() {
        let node = parse_toml(ALPHA).expect("parses");
        assert!(binding_problems(&node).is_empty());

        let bound = bind(&node, &[orders()]).expect("binds");

        assert_eq!(bound.receive_locations.len(), 1);
        assert_eq!(bound.receive_locations[0].name, "OrdersIn");
        assert_eq!(bound.receive_locations[0].address, "/var/xmip/in/orders");
        assert_eq!(
            bound.receive_locations[0].credentials.as_deref(),
            Some("orders-in")
        );
        assert!(bound.send_locations.is_empty(), "beta takes the Send Port");
        assert_eq!(bound.subscriptions.len(), 1);
        assert_eq!(bound.subscriptions[0].id, "billing");

        let beta = parse_toml(&ALPHA.replace("node_name = \"alpha\"", "node_name = \"beta\""))
            .expect("beta");
        let bound = bind(&beta, &[orders()]).expect("binds on beta");
        assert!(bound.receive_locations.is_empty());
        assert_eq!(bound.send_locations[0].name, "Billing");
    }

    #[test]
    fn an_unknown_application_is_refused() {
        let node =
            parse_toml(&ALPHA.replace("name = \"Orders\"", "name = \"Invoices\"")).expect("");
        let problems = bind(&node, &[orders()]).expect_err("refused");

        assert_eq!(
            problems,
            ["the node binds the Xmip Application 'Invoices', which it was not given"]
        );
    }

    #[test]
    fn a_location_the_application_does_not_declare_is_refused() {
        let node = parse_toml(&ALPHA.replace("name = \"OrdersIn\"", "name = \"Drop\"")).expect("");
        let problems = bind(&node, &[orders()]).expect_err("refused");

        assert_eq!(
            problems,
            [
                "the node binds the Receive Location 'Drop' of 'Orders', which 'Orders' does not \
              declare"
            ]
        );
    }

    #[test]
    fn a_bound_location_without_its_node_is_refused_by_the_reading() {
        let source = ALPHA.replacen("node = \"alpha\"\n", "", 1);
        let error = parse_toml(&source).expect_err("node is required");

        assert!(error.contains("node"), "{error}");
    }

    #[test]
    fn a_node_binding_nothing_reads_as_before() {
        let node = parse_toml(ALPHA.split("[[applications]]").next().expect("head")).expect("");

        assert!(node.applications.is_empty());
        assert_eq!(bind(&node, &[]).expect("nothing to bind"), Bound::default());
    }
}
