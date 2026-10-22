//! The cluster designer's views of the one `xmip.toml` (ADR-0064, amendment
//! 2026-10-03: *VS Code's Xmip Extension shall work against this one /
//! cluster toml file*, every designer a view of its own sections of it).
//!
//! [`Views::of`] reads a cluster's file and answers one view per artifact
//! kind, in the order the designer lists them ([`crate::kinds`]) — Cluster,
//! Node, Party, Module, Receive Port, Receive Location, Send Port, Send
//! Location, Send Port Group, Prepare, Promote, Demote, Route,
//! Transformation, Work Process, Correlation Set — each with the entries the file holds of
//! it: where each is ([`Entry::section`], the path an edit names it by,
//! [`crate::view_edit`]), whose it is (the cluster's, a node's, a binding's
//! or an Xmip Application's), and its values as the file writes them. A
//! Route entry is an Xmip Application held as a section ([`crate::section`])
//! with its routes as a graph ([`Routes`]). Beside the views, every compiled
//! unit the file's modules declare ([`Unit`]): what a call to one may name
//! and give.
//!
//! Which lists a table holds is the documents' shape: a node's document
//! ([`crate::XmipConfigurationDocument`]), its bindings
//! ([`crate::ApplicationBinding`]) and an Application
//! ([`crate::XmipApplication`]).

use serde::Serialize;
use toml::{Table, Value};

use crate::cluster::NODES;
use crate::field::{Field, entry_name, fields};
use crate::kinds::{Holder, KINDS, ROUTE, Reads, holds};
use crate::routes::Routes;
use crate::section::{ApplicationSection, SECTIONS};

/// What a cluster's file is, view by view.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Views {
    /// The cluster's `[service] cluster_name`; empty when it names none.
    pub cluster: String,
    pub views: Vec<View>,
    /// Where each table and key is written, what names what, and every key
    /// a table may hold: what an editor looks up and suggests by.
    pub written: Vec<crate::written::Written>,
    pub references: Vec<crate::references::Reference>,
    pub keys: &'static [crate::keys::Key],
    /// Every compiled unit the file's `[[modules]]` declare.
    pub units: Vec<Unit>,
}

/// One compiled unit a module of the file declares: what a call to it
/// names, what it can be called as, the stages it serves and what a call
/// gives, in the shape a technology's settings cross in.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Unit {
    pub module: String,
    pub name: String,
    pub capability: &'static str,
    pub stages: Vec<String>,
    /// Absent where the declaration does not read; its problem is the
    /// validation's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<&'static xcore::settings::Settings>,
}

/// The tables of the cluster's own that a node's document reads.
pub const CLUSTER_TABLES: [&str; 9] = [
    "service",
    "tuning",
    "runtime",
    "administration",
    "audit",
    "storage",
    "store",
    "operate",
    "provision",
];

/// One artifact kind and the entries the file holds of it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct View {
    /// `cluster`, `node`, `receive-port`, … `process`.
    pub kind: &'static str,
    /// As a person reads it: `Receive Port`.
    pub title: &'static str,
    /// What the designer says of the kind, where there is something to say.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<&'static str>,
    pub entries: Vec<Entry>,
    /// Where a new entry of this kind may be added, by the list's path.
    pub places: Vec<Place>,
}

/// One entry: a table of the file.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Entry {
    /// The path to its table: keys, and an entry of a list by its name.
    pub section: Vec<String>,
    pub name: String,
    /// Whose it is: `cluster`, a node's name, `binding <A>`, `<A>`.
    pub scope: String,
    /// Its values, a sub-table's by their dotted path.
    pub fields: Vec<Field>,
    /// What may be set on it ([`crate::offered`]), once the runtime has
    /// handed in what it owns ([`Views::offering`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub offered: Vec<crate::offered::Offered>,
    /// An Xmip Application's routes, on a Route entry that reads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routes: Option<Routes>,
    /// Why a Route entry does not read as an Application.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<String>,
}

/// A list a new entry may be added to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Place {
    /// The list's path, its last key the list.
    pub section: Vec<String>,
    pub scope: String,
}

/// A table that holds lists: where it is, whose, and what it is.
struct Container<'t> {
    section: Vec<String>,
    scope: String,
    holder: Holder,
    table: Option<&'t Table>,
}

impl Views {
    /// The views of the cluster's file `cluster`.
    ///
    /// # Errors
    /// The reader's words when the text is not TOML.
    pub fn of(cluster: &str) -> Result<Self, String> {
        let table: Table = cluster
            .parse()
            .map_err(|error| format!("{}: {error}", crate::PARSE_FAILED))?;
        let containers = containers(&table);
        let views = KINDS
            .iter()
            .map(|(kind, title, reads)| view(kind, title, reads, &table, &containers))
            .collect();
        Ok(Self {
            cluster: table
                .get("service")
                .and_then(|service| service.get("cluster_name"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            views,
            written: crate::written::written(cluster)?,
            references: crate::references::references(cluster)?,
            keys: crate::keys::keys(),
            units: units(&table),
        })
    }
}

fn view(
    kind: &'static str,
    title: &'static str,
    reads: &Reads,
    table: &Table,
    containers: &[Container<'_>],
) -> View {
    let (note, entries, places) = match reads {
        Reads::Cluster => (None, cluster_entries(table), Vec::new()),
        Reads::Nodes => {
            let entries = nodes(table)
                .map(|(name, own)| entry(vec![NODES.into(), name.clone()], name, name, own))
                .collect();
            (None, entries, Vec::new())
        }
        Reads::List(list) => {
            let (entries, places) = listed(list, containers);
            (None, entries, places)
        }
        Reads::Routes => routes(containers),
    };
    View {
        kind,
        title,
        note,
        entries,
        places,
    }
}

fn cluster_entries(table: &Table) -> Vec<Entry> {
    let empty = Table::new();
    CLUSTER_TABLES
        .iter()
        .map(|key| {
            let own = table.get(*key).and_then(Value::as_table).unwrap_or(&empty);
            entry(vec![(*key).to_string()], key, "cluster", own)
        })
        .collect()
}

/// Every unit the modules of the cluster and of each node declare, once
/// each by module and name; a module that does not read declares none here.
fn units(table: &Table) -> Vec<Unit> {
    let mut found: Vec<Unit> = Vec::new();
    for (_, module) in crate::modules::declared_in(table) {
        for unit in &module.manifest.units {
            if found
                .iter()
                .any(|seen| seen.module == module.name && seen.name == unit.name)
            {
                continue;
            }
            found.push(Unit {
                module: module.name.clone(),
                name: unit.name.clone(),
                capability: unit.capability.word(),
                stages: unit.stages.clone(),
                settings: crate::modules::declared(&module.name, unit).ok(),
            });
        }
    }
    found
}

fn nodes(table: &Table) -> impl Iterator<Item = (&String, &Table)> {
    table
        .get(NODES)
        .and_then(Value::as_table)
        .into_iter()
        .flat_map(|nodes| nodes.iter())
        .filter_map(|(name, own)| own.as_table().map(|own| (name, own)))
}

/// Every table that holds lists: the cluster's own, each node's, each
/// binding of either, and each Xmip Application held as a section.
fn containers(table: &Table) -> Vec<Container<'_>> {
    let mut found = vec![Container {
        section: Vec::new(),
        scope: "cluster".to_string(),
        holder: Holder::Node,
        table: Some(table),
    }];
    for (name, own) in nodes(table) {
        found.push(Container {
            section: vec![NODES.into(), name.clone()],
            scope: name.clone(),
            holder: Holder::Node,
            table: Some(own),
        });
    }
    let bindings: Vec<Container<'_>> = found
        .iter()
        .flat_map(|node| {
            named(node.table, "applications").map(move |(name, binding)| Container {
                section: [
                    node.section.clone(),
                    vec!["applications".into(), name.clone()],
                ]
                .concat(),
                scope: if node.section.is_empty() {
                    format!("binding {name}")
                } else {
                    format!("{}, binding {name}", node.scope)
                },
                holder: Holder::Binding,
                table: Some(binding),
            })
        })
        .collect();
    found.extend(bindings);
    for (name, section) in named(Some(table), SECTIONS) {
        found.push(Container {
            section: vec![SECTIONS.into(), name.clone()],
            scope: name,
            holder: Holder::Application,
            table: Some(section),
        });
    }
    found
}

/// The entries of `list` everywhere it is held, and where it may be.
fn listed(list: &str, containers: &[Container<'_>]) -> (Vec<Entry>, Vec<Place>) {
    let mut entries = Vec::new();
    let mut places = Vec::new();
    for container in containers.iter().filter(|c| holds(c.holder, list)) {
        let section = [container.section.clone(), vec![list.to_string()]].concat();
        for (name, own) in named(container.table, list) {
            let path = [section.clone(), vec![name.clone()]].concat();
            entries.push(entry(path, &name, &container.scope, own));
        }
        places.push(Place {
            section,
            scope: container.scope.clone(),
        });
    }
    (entries, places)
}

type Answer = (Option<&'static str>, Vec<Entry>, Vec<Place>);

/// Every Xmip Application held as a section, its routes drawn, and every
/// binding.
fn routes(containers: &[Container<'_>]) -> Answer {
    let mut entries = Vec::new();
    let mut places = vec![Place {
        section: vec![SECTIONS.into()],
        scope: "cluster".to_string(),
    }];
    for container in containers {
        let Some(own) = container.table else { continue };
        match container.holder {
            Holder::Application => {
                let section = ApplicationSection(own.clone());
                let mut drawn = entry(container.section.clone(), section.name(), "", own);
                drawn.scope = "Xmip Application".to_string();
                match section.application() {
                    Ok(document) => drawn.routes = Some(Routes::of(&document)),
                    Err(error) => drawn.problems.push(error),
                }
                entries.push(drawn);
            }
            Holder::Binding => {
                let name = container.section.last().cloned().unwrap_or_default();
                entries.push(entry(
                    container.section.clone(),
                    &name,
                    &container.scope,
                    own,
                ));
            }
            Holder::Node => places.push(Place {
                section: [container.section.clone(), vec!["applications".into()]].concat(),
                scope: container.scope.clone(),
            }),
        }
    }
    (Some(ROUTE), entries, places)
}

/// The entries of the list `key` in `table`, by their names.
fn named<'t>(table: Option<&'t Table>, key: &str) -> impl Iterator<Item = (String, &'t Table)> {
    table
        .and_then(|table| table.get(key))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.as_table().map(|own| (entry_name(own), own)))
}

fn entry(section: Vec<String>, name: &str, scope: &str, own: &Table) -> Entry {
    Entry {
        section,
        name: name.to_string(),
        scope: scope.to_string(),
        fields: fields(own),
        offered: Vec::new(),
        routes: None,
        problems: Vec::new(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::fixture::test_cluster;

    /// A cluster's file written from the test cluster's names, and its
    /// first two nodes: a shared Receive Location, the first node's own
    /// tuning, an Xmip Application held as a section and its binding.
    pub(crate) fn written() -> (String, String, String) {
        let cluster = test_cluster();
        let (first, second) = (cluster.node(0).name.clone(), cluster.node(1).name.clone());
        let text = format!(
            r#"# The cluster, as the designer is given it.
[service]
name = "xmip"
cluster_name = "{name}"

[tuning]
segments = 44

[[receive_locations]]
name = "drop"
start = true
transport = "xmip-core-transport-file"
address = "/var/xmip/in" # the shared drop

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
destination = {{ send-port = "Billing" }}
filter = "MessageType = 'Order'"

[[applications]]
name = "Orders"

[[applications.receive_locations]]
name = "OrdersIn"
node = "{first}"
start = true
transport = "xmip-core-transport-file"
address = "/var/xmip/in/orders"

[nodes.{first}.tuning]
receive_idle = "30s"

[nodes.{second}]
"#,
            name = cluster.name
        );
        (text, first, second)
    }

    fn view<'v>(views: &'v Views, kind: &str) -> &'v View {
        views
            .views
            .iter()
            .find(|view| view.kind == kind)
            .expect("a view")
    }

    fn said(view: &View) -> Vec<String> {
        view.entries
            .iter()
            .map(|entry| format!("{} | {}", entry.section.join("."), entry.scope))
            .collect()
    }

    #[test]
    fn every_kind_has_its_view_in_order() {
        let (text, _, _) = written();
        let views = Views::of(&text).expect("reads");

        let titles: Vec<_> = views.views.iter().map(|view| view.title).collect();
        assert_eq!(
            titles,
            [
                "Cluster",
                "Node",
                "Party",
                "Module",
                "Receive Port",
                "Receive Location",
                "Send Port",
                "Send Location",
                "Send Port Group",
                "Prepare",
                "Promote",
                "Demote",
                "Route",
                "Transformation",
                "Work Process",
                "Correlation Set",
            ]
        );
        let cluster = view(&views, "cluster");
        let tables: Vec<_> = cluster.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(tables, CLUSTER_TABLES);
        assert_eq!(views.cluster, test_cluster().name);
    }

    #[test]
    fn a_call_to_a_unit_is_an_entry_and_each_unit_is_said_with_its_declaration() {
        let (text, first, _) = written();
        let text = format!(
            "{text}\n[[nodes.{first}.modules]]\nname = \"orders\"\nstart = true\n\
             [nodes.{first}.modules.manifest]\ncapabilities = []\noffline = true\n\
             [nodes.{first}.modules.manifest.identity]\nname = \"orders\"\nversion = \"1\"\n\
             [nodes.{first}.modules.manifest.entrypoint]\n\
             [[nodes.{first}.modules.manifest.units]]\nname = \"Codec\"\n\
             capability = \"prepare\"\nstages = [\"receive\", \"send\"]\noffline = true\n\
             [[nodes.{first}.modules.manifest.units.settings]]\nname = \"format\"\n\
             kind = \"text\"\npresence = \"optional\"\n\n\
             [[xmip_applications.preparation_steps]]\nname = \"Unzip\"\nmodule = \"orders\"\n\
             unit = \"Codec\"\n"
        );
        let views = Views::of(&text).expect("reads");

        assert_eq!(
            said(view(&views, "prepare")),
            ["xmip_applications.Orders.preparation_steps.Unzip | Orders"]
        );
        let places: Vec<_> = view(&views, "transformation")
            .places
            .iter()
            .map(|p| p.section.join("."))
            .collect();
        assert!(
            places.contains(&"transformations".to_string()),
            "{places:?}"
        );
        assert!(places.contains(&"xmip_applications.Orders.transformations".to_string()));
        assert_eq!(said(view(&views, "module")).len(), 1);
        let unit = &views.units[0];
        assert_eq!(
            (unit.module.as_str(), unit.name.as_str()),
            ("orders", "Codec")
        );
        assert_eq!(unit.stages, ["receive", "send"]);
        assert_eq!(unit.settings.expect("declared").settings[0].name, "format");
    }
    #[test]
    fn the_cluster_and_each_node_are_entries_with_their_values() {
        let (text, first, second) = written();
        let views = Views::of(&text).expect("reads");

        let cluster = view(&views, "cluster");
        let tuning = cluster
            .entries
            .iter()
            .find(|e| e.name == "tuning")
            .expect("tuning");
        assert_eq!(tuning.fields[0].key, ["segments"]);
        assert_eq!(tuning.fields[0].value, "44");
        assert!(
            cluster
                .entries
                .iter()
                .any(|e| e.name == "store" && e.fields.is_empty())
        );

        let nodes = view(&views, "node");
        let mut names = [first.clone(), second];
        names.sort();
        assert_eq!(
            nodes.entries.iter().map(|e| &e.name).collect::<Vec<_>>(),
            names.iter().collect::<Vec<_>>()
        );
        let own = nodes
            .entries
            .iter()
            .find(|e| e.name == first)
            .expect("first");
        assert_eq!(own.fields[0].key, ["tuning", "receive_idle"]);
        assert_eq!(own.fields[0].value, "\"30s\"");
    }

    #[test]
    fn a_receive_location_is_found_wherever_the_file_holds_one() {
        let (text, first, _) = written();
        let views = Views::of(&text).expect("reads");
        let locations = view(&views, "receive-location");

        assert_eq!(
            said(locations),
            [
                "receive_locations.drop | cluster",
                "applications.Orders.receive_locations.OrdersIn | binding Orders",
                "xmip_applications.Orders.receive_locations.OrdersIn | Orders",
            ]
        );
        let places: Vec<_> = locations
            .places
            .iter()
            .map(|p| p.section.join("."))
            .collect();
        assert!(
            places.contains(&"receive_locations".to_string()),
            "{places:?}"
        );
        assert!(
            places.contains(&format!("nodes.{first}.receive_locations")),
            "{places:?}"
        );
        assert!(places.contains(&"applications.Orders.receive_locations".to_string()));
        assert!(places.contains(&"xmip_applications.Orders.receive_locations".to_string()));
        assert_eq!(said(view(&views, "send-location")), [] as [String; 0]);
    }

    #[test]
    fn a_route_entry_is_an_application_with_its_routes_and_its_bindings() {
        let (text, _, _) = written();
        let views = Views::of(&text).expect("reads");
        let route = view(&views, "route");

        assert_eq!(
            said(route),
            [
                "applications.Orders | binding Orders",
                "xmip_applications.Orders | Xmip Application"
            ]
        );
        let routes = route.entries[1].routes.as_ref().expect("drawn");
        let ids: Vec<_> = routes.nodes.iter().map(|node| node.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "receive-location:OrdersIn",
                "subscription:billing",
                "send-port:Billing"
            ]
        );
        assert_eq!(route.places[0].section, [SECTIONS]);

        let broken = text.replace(
            "[[xmip_applications.send_ports]]",
            "[[xmip_applications.send_port]]",
        );
        let broken = Views::of(&broken).expect("reads");
        let route = view(&broken, "route");
        assert!(route.entries[1].routes.is_none());
        assert!(
            route.entries[1].problems[0].contains("send_port"),
            "{:?}",
            route.entries[1].problems
        );
    }

    #[test]
    fn a_receive_port_is_an_entry_of_its_application_and_added_there() {
        let (text, _, _) = written();
        let views = Views::of(&text).expect("reads");
        let ports = view(&views, "receive-port");

        assert!(ports.note.is_none());
        assert_eq!(
            said(ports),
            ["xmip_applications.Orders.receive_ports.Orders | Orders"]
        );
        let places: Vec<_> = ports.places.iter().map(|p| p.section.join(".")).collect();
        assert_eq!(places, ["xmip_applications.Orders.receive_ports"]);
        let location = &view(&views, "receive-location").entries[2];
        assert!(
            location
                .fields
                .iter()
                .any(|f| f.key == ["receive_port"] && f.value == "\"Orders\""),
            "{:?}",
            location.fields
        );
    }
}
