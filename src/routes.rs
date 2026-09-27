//! An Xmip Application's routes as a graph, the model the routes designer
//! draws (ADR-0064): what is there, what connects to what, and what a
//! Subscription may be connected to.
//!
//! Nodes are the Application's Receive Locations, Subscriptions, Xmip
//! Processes, Send Port Groups and Send Ports; each carries its place along
//! the route, left to right, so a designer lays them out without knowing
//! what any of them is. Edges are what the runtime does: every Receive
//! Location publishes what it receives and every Subscription is asked
//! (`publishes`), a Subscription routes a match to its destination
//! (`routes`), and a Send Port Group gathers its Send Ports (`gathers`).

use path::expression::OPERATORS;
use route::Subscriber;
use serde::Serialize;

use crate::application::XmipApplicationDocument;
use crate::filter::{self, FilterPart};

/// The graph of one Application's routes, and the words its filters use.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Routes {
    pub application: String,
    pub nodes: Vec<RouteNode>,
    pub edges: Vec<RouteEdge>,
    /// What is wrong with the design, one sentence each; drawn as it is.
    pub problems: Vec<String>,
    /// The operators a filter row offers: the expression language's own
    /// (`path::expression::OPERATORS`).
    pub operators: Vec<&'static str>,
    /// The kinds a compared value is read as, [`filter::kinds`].
    pub kinds: Vec<&'static str>,
}

/// One thing the Application declares.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RouteNode {
    /// Unique in the graph; what an edit names it by.
    pub id: String,
    /// `receive-location`, `subscription`, `xmip-process`, `send-port-group`
    /// or `send-port`.
    pub kind: &'static str,
    pub name: String,
    /// Its place along the route, left to right, from 0.
    pub column: u32,
    /// Whether a Subscription may route here.
    pub target: bool,
    /// A Subscription's filter as rows and groups.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<FilterPart>,
    /// A Subscription's filter as the Application writes it: one line of
    /// the expression language, as written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// One connection, by the nodes' ids.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RouteEdge {
    pub from: String,
    pub to: String,
    /// `publishes`, `routes` or `gathers`.
    pub kind: &'static str,
}

const RECEIVE_LOCATION: &str = "receive-location";
const SUBSCRIPTION: &str = "subscription";
const XMIP_PROCESS: &str = "xmip-process";
const SEND_PORT_GROUP: &str = "send-port-group";
const SEND_PORT: &str = "send-port";

/// The id of the node `kind` names `name` by.
#[must_use]
pub fn node_id(kind: &str, name: &str) -> String {
    format!("{kind}:{name}")
}

/// The id of a Subscription's node, for an edit to name it by.
#[must_use]
pub fn subscription_id(id: &str) -> String {
    node_id(SUBSCRIPTION, id)
}

/// The node a destination is drawn as.
#[must_use]
pub fn destination_id(destination: &Subscriber) -> String {
    let kind = match destination {
        Subscriber::Process(_) => XMIP_PROCESS,
        Subscriber::SendPort(_) => SEND_PORT,
        Subscriber::SendGroup(_) => SEND_PORT_GROUP,
    };
    node_id(kind, destination.name())
}

/// The destination the node `id` is, when a Subscription may route to it
/// and the Application declares it.
#[must_use]
pub fn destination_of(document: &XmipApplicationDocument, id: &str) -> Option<Subscriber> {
    let (kind, name) = id.split_once(':')?;
    let destination = match kind {
        XMIP_PROCESS => Subscriber::Process(name.to_string()),
        SEND_PORT => Subscriber::SendPort(name.to_string()),
        SEND_PORT_GROUP => Subscriber::SendGroup(name.to_string()),
        _ => return None,
    };
    document.declares(&destination).then_some(destination)
}

impl Routes {
    /// The graph of `document`'s routes, drawn whether or not it is sound.
    #[must_use]
    pub fn of(document: &XmipApplicationDocument) -> Self {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        let plain = |kind: &'static str, name: &str, column: u32, target: bool| RouteNode {
            id: node_id(kind, name),
            kind,
            name: name.to_string(),
            column,
            target,
            filter: None,
            summary: None,
        };

        for location in &document.receive_locations {
            nodes.push(plain(RECEIVE_LOCATION, &location.name, 0, false));
        }
        for subscription in &document.subscriptions {
            nodes.push(RouteNode {
                filter: Some(filter::rows(&subscription.filter)),
                summary: Some(subscription.filter.text().to_string()),
                ..plain(SUBSCRIPTION, &subscription.id, 1, false)
            });
            for location in &document.receive_locations {
                edges.push(edge(
                    RECEIVE_LOCATION,
                    &location.name,
                    subscription_id(&subscription.id),
                    "publishes",
                ));
            }
            edges.push(RouteEdge {
                from: subscription_id(&subscription.id),
                to: destination_id(&subscription.destination),
                kind: "routes",
            });
        }
        for process in &document.xmip_processes {
            nodes.push(plain(XMIP_PROCESS, &process.name, 2, true));
        }
        for group in &document.send_port_groups {
            nodes.push(plain(SEND_PORT_GROUP, &group.name, 2, true));
            for member in &group.send_ports {
                edges.push(edge(
                    SEND_PORT_GROUP,
                    &group.name,
                    node_id(SEND_PORT, member),
                    "gathers",
                ));
            }
        }
        for port in &document.send_ports {
            nodes.push(plain(SEND_PORT, &port.name, 3, true));
        }

        Self {
            application: document.application.name.clone(),
            nodes,
            edges,
            problems: document.problems(),
            operators: OPERATORS.to_vec(),
            kinds: filter::kinds(),
        }
    }
}

fn edge(kind: &str, name: &str, to: String, relation: &'static str) -> RouteEdge {
    RouteEdge {
        from: node_id(kind, name),
        to,
        kind: relation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_application;

    const ORDERS: &str = r#"[application]
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
filter = "MessageType = 'Order'"

[[subscriptions]]
id = "books"
destination = { send-group = "Books" }
filter = "true"
"#;

    #[test]
    fn the_graph_holds_every_part_and_what_routes_where() {
        let routes = Routes::of(&parse_application(ORDERS).expect("parses"));

        assert_eq!(routes.application, "Orders");
        let ids = routes
            .nodes
            .iter()
            .map(|n| n.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            [
                "receive-location:OrdersIn",
                "subscription:billing",
                "subscription:books",
                "xmip-process:Approval",
                "send-port-group:Books",
                "send-port:Billing",
                "send-port:Ledger",
            ]
        );
        let columns = routes.nodes.iter().map(|n| n.column).collect::<Vec<_>>();
        assert_eq!(columns, [0, 1, 1, 2, 2, 3, 3]);

        let said = routes
            .edges
            .iter()
            .map(|e| format!("{} {} {}", e.from, e.kind, e.to))
            .collect::<Vec<_>>();
        assert!(said.contains(&"receive-location:OrdersIn publishes subscription:billing".into()));
        assert!(said.contains(&"subscription:billing routes send-port:Billing".into()));
        assert!(said.contains(&"subscription:books routes send-port-group:Books".into()));
        assert!(said.contains(&"send-port-group:Books gathers send-port:Ledger".into()));
        assert_eq!(said.len(), 6);

        let billing = &routes.nodes[1];
        assert_eq!(billing.summary.as_deref(), Some("MessageType = 'Order'"));
        assert!(billing.filter.is_some());
        assert!(!billing.target && routes.nodes[3].target && routes.nodes[6].target);
        assert!(routes.problems.is_empty());
        assert_eq!(routes.operators[0], "=");
        assert_eq!(routes.kinds, ["text", "integer", "boolean", "expression"]);
    }

    #[test]
    fn a_node_id_names_a_destination_only_when_it_is_one_and_declared() {
        let document = parse_application(ORDERS).expect("parses");

        assert_eq!(
            destination_of(&document, "xmip-process:Approval"),
            Some(Subscriber::Process("Approval".to_string()))
        );
        assert_eq!(destination_of(&document, "send-port:Audit"), None);
        assert_eq!(destination_of(&document, "receive-location:OrdersIn"), None);
        assert_eq!(destination_of(&document, "nonsense"), None);
    }
}
