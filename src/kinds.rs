//! The artifact kinds the designer shows, in its order, and what each reads
//! of the cluster's file (ADR-0064, amendments 2026-10-03 and 2026-10-22).
//!
//! Every kind is a list or table the configuration defines. Prepare,
//! Promote, Demote, Transformation and Debatch are calls to compiled units
//! ([`crate::step`]), each listed where the file holds it; a Module is a
//! `[[modules]]` entry, whose manifest declares the units the calls name;
//! a Correlation is a `[[correlations]]` entry ([`crate::correlation`]).

/// What a kind reads.
pub(crate) enum Reads {
    /// The cluster's own tables.
    Cluster,
    /// Each `[nodes.<name>]`.
    Nodes,
    /// The entries of a list, wherever the file holds it.
    List(&'static str),
    /// Each Xmip Application section with its routes, and each binding.
    Routes,
}

/// What the designer says of the Route kind.
pub(crate) const ROUTE: &str = "Each Xmip Application held in this file, with its routes, and \
                                the bindings that run it.";

/// Every kind: its word, its title and what it reads.
pub(crate) const KINDS: [(&str, &str, Reads); 17] = [
    ("cluster", "Cluster", Reads::Cluster),
    ("node", "Node", Reads::Nodes),
    ("party", "Party", Reads::List("parties")),
    ("module", "Module", Reads::List("modules")),
    ("receive-port", "Receive Port", Reads::List("receive_ports")),
    (
        "receive-location",
        "Receive Location",
        Reads::List("receive_locations"),
    ),
    ("send-port", "Send Port", Reads::List("send_ports")),
    (
        "send-location",
        "Send Location",
        Reads::List("send_locations"),
    ),
    (
        "send-port-group",
        "Send Port Group",
        Reads::List("send_port_groups"),
    ),
    ("prepare", "Prepare", Reads::List("preparation_steps")),
    ("promote", "Promote", Reads::List("promotions")),
    ("demote", "Demote", Reads::List("demotions")),
    ("route", "Route", Reads::Routes),
    (
        "transformation",
        "Transformation",
        Reads::List("transformations"),
    ),
    ("debatch", "Debatch", Reads::List("debatches")),
    (
        "work-process",
        "Work Process",
        Reads::List("work_processes"),
    ),
    ("correlation", "Correlation", Reads::List("correlations")),
];

/// The tables that hold lists: a node's document, a binding, and an Xmip
/// Application.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Holder {
    Node,
    Binding,
    Application,
}

/// The calls to compiled units, which a node's document and an Application
/// both hold.
const STEPS: [&str; 5] = [
    "preparation_steps",
    "promotions",
    "demotions",
    "transformations",
    "debatches",
];

/// Which lists each holder holds: the documents' shape.
pub(crate) fn holds(holder: Holder, list: &str) -> bool {
    let lists: &[&str] = match holder {
        Holder::Node => &[
            "receive_locations",
            "send_locations",
            "work_processes",
            "applications",
            "parties",
            "modules",
            "correlations",
        ],
        Holder::Binding => &["receive_locations", "send_ports"],
        Holder::Application => &[
            "receive_ports",
            "receive_locations",
            "work_processes",
            "send_ports",
            "send_port_groups",
            "subscriptions",
        ],
    };
    lists.contains(&list) || holder != Holder::Binding && STEPS.contains(&list)
}
