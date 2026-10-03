//! A node's binding of an Xmip Application (ADR-0064): that the node runs
//! it, and the environment's side of what it declares.
//!
//! The Application holds the design, drawn once as a section of the
//! cluster's `xmip.toml`, `[[xmip_applications]]` ([`crate::section`]);
//! each node's configuration names the Applications it runs in
//! `[[applications]]`, each binding the section of its name, and gives every
//! Receive Location and Send Port it binds a transport, an address, a
//! reference to its credentials and the node that takes it, as the bindings
//! of the platforms Xmip replaces do. The same route is never written again
//! per node.
//!
//! ```toml
//! [[applications]]
//! name = "Orders"
//!
//! [[applications.receive_locations]]
//! name = "OrdersIn"
//! node = "<node>"
//! start = true
//! transport = "xmip-core-transport-file"
//! address = "/var/xmip/in/orders"
//! ```
//!
//! [`binding_problems`] checks what a binding says on its own, its
//! section's design with it; [`bind`] joins it to the Applications it names
//! and takes what one node runs.

use std::collections::BTreeSet;

use route::Subscription;
use serde::{Deserialize, Serialize};

use crate::application::{SendPortGroup, XmipApplication};
use crate::port::{DesignedReceiveLocation, DesignedSendPort};
use crate::section::ApplicationSection;
use crate::{ConfiguredLocation, XmipConfigurationDocument};

/// `[[applications]]`: one Xmip Application this node runs, the section of
/// its name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationBinding {
    /// The Application's name, its section's `name`.
    pub name: String,
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

/// A Receive Port of a bound Application, with the Receive Locations at it
/// this node takes, as designed: their interaction and depth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundReceivePort {
    /// The Application that declares it.
    pub application: String,
    pub name: String,
    pub receive_locations: Vec<DesignedReceiveLocation>,
}

/// What one node runs of the Applications it binds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bound {
    /// The bound Receive Locations this node takes.
    pub receive_locations: Vec<ConfiguredLocation>,
    /// The Receive Ports those Locations are at.
    pub receive_ports: Vec<BoundReceivePort>,
    /// The bound Send Ports this node takes, as the Send Locations they are.
    pub send_locations: Vec<ConfiguredLocation>,
    /// Those Send Ports as designed, with their policy.
    pub send_ports: Vec<DesignedSendPort>,
    /// Every Subscription of every bound Application.
    pub subscriptions: Vec<Subscription>,
    /// Every Send Port Group of every bound Application: the Send Ports a
    /// Subscription routed to the group reaches together.
    pub send_port_groups: Vec<SendPortGroup>,
}

/// What the bindings in `document` say wrong: a name, the section of that
/// name held and sound, every bound Location's node, name and transport
/// present, and no Location bound twice.
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
        section_problems(document, application, &mut problems);

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

/// The `[[xmip_applications]]` section of `document` named `name`.
#[must_use]
pub fn section<'a>(
    document: &'a XmipConfigurationDocument,
    name: &str,
) -> Option<&'a ApplicationSection> {
    document
        .xmip_applications
        .iter()
        .find(|section| section.name() == name)
}

/// A binding binds the section of its name: it must be there, read as an
/// Application and be sound.
fn section_problems(
    document: &XmipConfigurationDocument,
    application: &str,
    problems: &mut Vec<String>,
) {
    let Some(found) = section(document, application) else {
        problems.push(format!(
            "the configuration binds the Xmip Application '{application}' and holds no \
             [[xmip_applications]] '{application}'"
        ));
        return;
    };
    match found.application() {
        Ok(design) => problems.extend(
            design
                .problems()
                .into_iter()
                .map(|problem| format!("Xmip Application '{application}': {problem}")),
        ),
        Err(error) => problems.push(error),
    }
}

/// Join the node's bindings to the Applications it was given and take what
/// this node runs: the bound Locations whose `node` is this node's name,
/// the Receive Ports they are at and the Send Ports they are, and every
/// Subscription and Send Port Group of every bound Application.
///
/// # Errors
/// Every problem found, one sentence each: a binding's own
/// ([`binding_problems`]), an Application the node binds and was not given,
/// a bound Application's own problems, a Location it does not declare, and
/// a Subscription or Location name two of them share on this node.
pub fn bind(
    document: &XmipConfigurationDocument,
    applications: &[XmipApplication],
) -> Result<Bound, Vec<String>> {
    let mut problems = binding_problems(document);
    let node = &document.service.node_name;
    let mut bound = Bound::default();

    for binding in &document.applications {
        let name = &binding.name;
        let Some(application) = applications.iter().find(|a| &a.name == name) else {
            problems.push(format!(
                "the node binds the Xmip Application '{name}', which it was not given"
            ));
            continue;
        };

        for problem in application.problems() {
            problems.push(format!("Xmip Application '{name}': {problem}"));
        }

        let receive = taken(
            &binding.receive_locations,
            &application.receive_locations,
            |design| &design.name,
            node,
        );
        let send = taken(
            &binding.send_ports,
            &application.send_ports,
            |design| &design.name,
            node,
        );
        for (what, undeclared) in [
            ("Receive Location", &receive.undeclared),
            ("Send Port", &send.undeclared),
        ] {
            for wanted in undeclared {
                problems.push(format!(
                    "the node binds the {what} '{wanted}' of '{name}', which '{name}' does not \
                     declare"
                ));
            }
        }
        bound.receive_locations.extend(receive.locations);
        bound.send_locations.extend(send.locations);
        let (receive, send): (Vec<&DesignedReceiveLocation>, Vec<&DesignedSendPort>) =
            (receive.designs, send.designs);

        for port in &application.receive_ports {
            let at: Vec<DesignedReceiveLocation> = receive
                .iter()
                .filter(|design| design.receive_port.as_deref() == Some(port.name.as_str()))
                .map(|design| (*design).clone())
                .collect();
            if !at.is_empty() {
                bound.receive_ports.push(BoundReceivePort {
                    application: name.clone(),
                    name: port.name.clone(),
                    receive_locations: at,
                });
            }
        }
        bound.send_ports.extend(send.into_iter().cloned());
        bound
            .subscriptions
            .extend(application.subscriptions.iter().cloned());
        bound
            .send_port_groups
            .extend(application.send_port_groups.iter().cloned());
    }

    once_on_the_node(document, &bound, &mut problems);

    if problems.is_empty() {
        Ok(bound)
    } else {
        Err(problems)
    }
}

/// What a node takes of the bound Locations of one kind.
struct Taken<'a, T> {
    /// The bound Locations whose `node` is this node.
    locations: Vec<ConfiguredLocation>,
    /// Their designs, in the same order.
    designs: Vec<&'a T>,
    /// The names bound that the Application does not declare.
    undeclared: Vec<String>,
}

fn taken<'a, T>(
    locations: &[BoundLocation],
    declared: &'a [T],
    name_of: impl Fn(&T) -> &String,
    node: &str,
) -> Taken<'a, T> {
    let mut taken = Taken {
        locations: Vec::new(),
        designs: Vec::new(),
        undeclared: Vec::new(),
    };
    for location in locations {
        let wanted = &location.location.name;
        match declared.iter().find(|design| name_of(design) == wanted) {
            Some(design) if location.node == node => {
                taken.locations.push(location.location.clone());
                taken.designs.push(design);
            }
            Some(_) => {}
            None => taken.undeclared.push(wanted.clone()),
        }
    }
    taken
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
pub(crate) mod tests {
    use super::*;
    use crate::fixture::test_cluster;
    use crate::parse_toml;

    /// The Orders Application as its section.
    pub(crate) const ORDERS: &str = r#"
[[xmip_applications]]
name = "Orders"

[[xmip_applications.receive_ports]]
name = "Orders"

[[xmip_applications.receive_locations]]
name = "OrdersIn"
receive_port = "Orders"
interaction = "data-transfer"
depth = "light"

[[xmip_applications.send_ports]]
name = "Billing"

[[xmip_applications.subscriptions]]
id = "billing"
destination = { send-port = "Billing" }
filter = "MessageType = 'Order'"
"#;

    /// The test cluster's node at `place`, binding Orders: the first node
    /// takes the Receive Location, the second the Send Port.
    pub(crate) fn binding(place: usize) -> String {
        let cluster = test_cluster();
        let (first, second) = (&cluster.node(0).name, &cluster.node(1).name);
        format!(
            "[service]\nname = \"xmip\"\ncluster_name = \"{}\"\nnode_name = \"{}\"\n\n\
             [[applications]]\nname = \"Orders\"\n\n\
             [[applications.receive_locations]]\nname = \"OrdersIn\"\nnode = \"{first}\"\n\
             start = true\ntransport = \"xmip-core-transport-file\"\n\
             address = \"/var/xmip/in/orders\"\ncredentials = \"orders-in\"\n\n\
             [[applications.send_ports]]\nname = \"Billing\"\nnode = \"{second}\"\n\
             start = true\ntransport = \"xmip-core-transport-http\"\n\
             address = \"https://billing.example/orders\"\n{ORDERS}",
            cluster.name,
            cluster.node(place).name
        )
    }

    fn node(text: &str) -> XmipConfigurationDocument {
        parse_toml(text).expect("reads")
    }

    fn orders(document: &XmipConfigurationDocument) -> XmipApplication {
        section(document, "Orders")
            .expect("held")
            .application()
            .expect("reads")
    }

    #[test]
    fn a_binding_binds_what_this_node_takes_and_every_subscription() {
        let first = node(&binding(0));
        assert!(
            binding_problems(&first).is_empty(),
            "{:?}",
            binding_problems(&first)
        );

        let bound = bind(&first, &[orders(&first)]).expect("binds");

        assert_eq!(bound.receive_locations.len(), 1);
        assert_eq!(bound.receive_locations[0].name, "OrdersIn");
        assert_eq!(bound.receive_locations[0].address, "/var/xmip/in/orders");
        assert_eq!(
            bound.receive_locations[0].credentials.as_deref(),
            Some("orders-in")
        );
        assert_eq!(bound.receive_ports.len(), 1);
        assert_eq!(bound.receive_ports[0].name, "Orders");
        assert_eq!(bound.receive_ports[0].receive_locations[0].name, "OrdersIn");
        assert!(bound.send_locations.is_empty(), "the second node takes it");
        assert!(bound.send_ports.is_empty());
        assert_eq!(bound.subscriptions.len(), 1);
        assert_eq!(bound.subscriptions[0].id, "billing");

        let second = node(&binding(1));
        let bound = bind(&second, &[orders(&second)]).expect("binds on the second");
        assert!(bound.receive_locations.is_empty());
        assert!(bound.receive_ports.is_empty());
        assert_eq!(bound.send_locations[0].name, "Billing");
        assert_eq!(bound.send_ports[0].name, "Billing");
    }

    #[test]
    fn a_bound_applications_send_port_groups_come_with_its_subscriptions() {
        let text = format!(
            "{}\n[[xmip_applications.send_port_groups]]\nname = \"Everyone\"\n\
             send_ports = [\"Billing\"]\n",
            binding(0)
        );
        let first = node(&text);

        let bound = bind(&first, &[orders(&first)]).expect("binds");

        assert_eq!(bound.send_port_groups.len(), 1);
        assert_eq!(bound.send_port_groups[0].send_ports, ["Billing"]);
    }

    #[test]
    fn an_application_not_given_is_refused() {
        let first = node(&binding(0));
        let problems = bind(&first, &[]).expect_err("refused");

        assert_eq!(
            problems,
            ["the node binds the Xmip Application 'Orders', which it was not given"]
        );
    }

    #[test]
    fn a_location_the_application_does_not_declare_is_refused() {
        let first = node(&binding(0).replacen("name = \"OrdersIn\"", "name = \"Drop\"", 1));
        let problems = bind(&first, &[orders(&first)]).expect_err("refused");

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
        let first = test_cluster().node(0).name.clone();
        let source = binding(0).replacen(&format!("node = \"{first}\"\n"), "", 1);
        let error = parse_toml(&source).expect_err("node is required");

        assert!(error.contains("node"), "{error}");
    }

    #[test]
    fn a_node_binding_nothing_reads_as_before() {
        let text = binding(0);
        let first = node(text.split("[[applications]]").next().expect("head"));

        assert!(first.applications.is_empty());
        assert_eq!(
            bind(&first, &[]).expect("nothing to bind"),
            Bound::default()
        );
    }

    #[test]
    fn a_missing_or_unsound_section_is_a_problem_of_the_binding() {
        let text = binding(0);
        let missing = binding_problems(&node(&text.replace(ORDERS, "")));
        assert_eq!(missing.len(), 1, "{missing:?}");
        assert!(
            missing[0].contains("no [[xmip_applications]] 'Orders'"),
            "{missing:?}"
        );

        let unsound = binding_problems(&node(
            &text.replace("[[xmip_applications.send_ports]]\nname = \"Billing\"\n", ""),
        ));
        assert_eq!(unsound.len(), 1, "{unsound:?}");
        assert!(
            unsound[0].starts_with("Xmip Application 'Orders': "),
            "{unsound:?}"
        );
    }
}
