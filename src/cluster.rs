//! A cluster's configuration, `xmip.toml`, and the one slicing of it into
//! each node's (ADR-0031, amendment 2026-10-03: *There is one xmip.toml
//! file per cluster. When deployed the sections regarding a node will be
//! sliced to that node*).
//!
//! What the whole cluster shares is written once, at the top; what concerns
//! one node is under `[nodes.<name>]`, in the same sections a node's own
//! document has:
//!
//! ```toml
//! [service]
//! name = "xmip"
//! cluster_name = "<cluster>"
//!
//! [tuning]
//! segments = 44
//!
//! [nodes.<node>.service]
//! name = "xmip-<node>"
//!
//! [nodes.<node>.tuning]
//! receive_threads_per_hardware_thread = 4
//!
//! [nodes.<another>.runtime]
//! storage = "postgresql"
//! connection = "host=db-1.example dbname=xmip_runtime user=xmip_storage"
//!
//! [nodes.<another>.storage.database]
//! password = "xmip-storage-database"
//! ```
//!
//! The names are the cluster's own, chosen by whoever runs it; `<node>`
//! and `<another>` stand for two.
//!
//! [`slice`] writes one node its document: the shared sections and the
//! node's own, the node's value winning where both say one. Tables merge
//! key by key; an array of tables that each have a `name` —
//! `[[receive_locations]]`, `[[applications]]` — merges entry by entry on
//! the name, an entry the node names that the cluster does not being added;
//! any other value the node gives replaces the cluster's. `[service]
//! node_name` is the node's key under `[nodes]`, written by the slice and
//! never by hand. An Xmip Application held as a section,
//! `[[xmip_applications]]` ([`crate::section`]), is given only to the
//! nodes whose bindings name it. Desired state calls it as each node is deployed
//! (`deployment-model.md` section 8), and the runtime's validation calls it
//! for every node the file declares.

use toml::{Table, Value};

use crate::section::SECTIONS;

/// The table a cluster's file declares its nodes in.
pub const NODES: &str = "nodes";

/// The names of the nodes the cluster's file `cluster` declares, in the
/// order of their names.
///
/// # Errors
/// When the text is not TOML or `[nodes]` is not a table of tables.
pub fn nodes(cluster: &str) -> Result<Vec<String>, String> {
    let table = read(cluster)?;
    Ok(declared(&table)?.keys().cloned().collect())
}

/// The key a node's table declares its roles under: `node::NodeRole`'s
/// words, by comma or plus (ADR-0056, amendment 2026-10-01).
pub const ROLES: &str = "roles";

/// Each node the cluster's file `cluster` declares, in the order of their
/// names, with the role words its [`ROLES`] says, as written — none where
/// it says none. Whether a word is a role is `node::NodeRole`'s to say.
///
/// # Errors
/// As [`nodes`], and where a node's [`ROLES`] is not a string.
pub fn roles(cluster: &str) -> Result<Vec<(String, Vec<String>)>, String> {
    let table = read(cluster)?;
    declared(&table)?
        .into_iter()
        .map(|(node, own)| {
            let words = match own.get(ROLES) {
                None => Vec::new(),
                Some(Value::String(said)) => said
                    .split([',', '+'])
                    .map(str::trim)
                    .filter(|word| !word.is_empty())
                    .map(str::to_string)
                    .collect(),
                Some(_) => {
                    return Err(format!("[nodes.{node}] {ROLES} is a string of role words"));
                }
            };
            Ok((node, words))
        })
        .collect()
}

/// Whether `source` is a cluster's file: it declares its nodes.
#[must_use]
pub fn is_cluster(source: &str) -> bool {
    source
        .parse::<Table>()
        .is_ok_and(|table| table.contains_key(NODES))
}

/// The node `node`'s configuration document, sliced from the cluster's
/// file `cluster`: what [`crate::parse_toml`] reads.
///
/// # Errors
/// When the text is not TOML, the cluster declares no node `node`, the
/// cluster's `[service]` names a `node_name`, or the node's own names
/// another than its key.
pub fn slice(cluster: &str, node: &str) -> Result<String, String> {
    let mut table = read(cluster)?;
    let mut own = {
        let declared = declared(&table)?;
        match declared.get(node) {
            Some(Value::Table(own)) => own.clone(),
            _ => {
                // Said in the file's own tables, as every problem this
                // crate finds in a document is: no node is located yet.
                let tables: Vec<String> = declared
                    .keys()
                    .map(|name| format!("[nodes.{name}]"))
                    .collect();
                return Err(format!(
                    "the cluster's xmip.toml has no [nodes.{node}]; it has {}",
                    if tables.is_empty() {
                        "none".to_string()
                    } else {
                        tables.join(", ")
                    }
                ));
            }
        }
    };
    table.remove(NODES);
    if let Some(named) = node_name(&table) {
        return Err(format!(
            "the cluster's [service] names the node_name '{named}'; a node's name is its key \
             under [nodes]"
        ));
    }
    if let Some(named) = node_name(&own).filter(|named| *named != node) {
        return Err(format!(
            "[nodes.{node}.service] names the node_name '{named}'; a node's name is its key \
             under [nodes], '{node}'"
        ));
    }
    if let Some(Value::Table(service)) = own.get_mut("service") {
        service.remove("node_name");
    }
    merge(&mut table, own);
    bound_sections(&mut table);
    match table
        .entry("service")
        .or_insert_with(|| Value::Table(Table::new()))
    {
        Value::Table(service) => {
            service.insert("node_name".to_string(), Value::String(node.to_string()));
        }
        _ => return Err("[service] is not a table".to_string()),
    }
    let written = toml::to_string(&table).map_err(|error| error.to_string())?;
    Ok(format!(
        "# The node {node}'s configuration, sliced from its cluster's xmip.toml.\n\
         # Edit the cluster's file; this one is written again as the node is deployed.\n\n\
         {written}"
    ))
}

/// Every node's document the cluster's file declares, by name: what a
/// surface validates a cluster's file by, one node at a time.
///
/// # Errors
/// As [`slice`], for the first node that does not slice.
pub fn slices(cluster: &str) -> Result<Vec<(String, String)>, String> {
    nodes(cluster)?
        .into_iter()
        .map(|node| slice(cluster, &node).map(|document| (node, document)))
        .collect()
}

fn read(cluster: &str) -> Result<Table, String> {
    cluster
        .parse::<Table>()
        .map_err(|error| format!("{}: {error}", crate::PARSE_FAILED))
}

fn declared(table: &Table) -> Result<Table, String> {
    match table.get(NODES) {
        None => Ok(Table::new()),
        Some(Value::Table(nodes)) if nodes.values().all(Value::is_table) => Ok(nodes.clone()),
        Some(_) => Err("[nodes] holds one table per node, [nodes.<name>]".to_string()),
    }
}

/// A node is given the Xmip Applications its bindings name and no other
/// (ADR-0064, amendment 2026-10-03: the sections are sliced to the nodes
/// that bind them).
fn bound_sections(table: &mut Table) {
    let bound: Vec<String> = match table.get("applications") {
        Some(Value::Array(bindings)) => bindings
            .iter()
            .filter_map(|binding| name(binding).map(str::to_string))
            .collect(),
        _ => Vec::new(),
    };
    let emptied = match table.get_mut(SECTIONS) {
        Some(Value::Array(sections)) => {
            sections.retain(|section| name(section).is_some_and(|n| bound.iter().any(|b| b == n)));
            sections.is_empty()
        }
        _ => false,
    };
    if emptied {
        table.remove(SECTIONS);
    }
}

fn node_name(table: &Table) -> Option<&str> {
    table.get("service")?.get("node_name")?.as_str()
}

/// `over` written onto `base`, `over`'s value winning.
fn merge(base: &mut Table, over: Table) {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(Value::Table(shared)), Value::Table(own)) => merge(shared, own),
            (Some(Value::Array(shared)), Value::Array(own)) if named(shared) && named(&own) => {
                for entry in own {
                    let found = shared.iter_mut().find(|held| name(held) == name(&entry));
                    match (found, entry) {
                        (Some(Value::Table(held)), Value::Table(entry)) => merge(held, entry),
                        (_, entry) => shared.push(entry),
                    }
                }
            }
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

fn name(entry: &Value) -> Option<&str> {
    entry.get("name")?.as_str()
}

/// An array of tables each with a `name`: merged entry by entry.
fn named(array: &[Value]) -> bool {
    array
        .iter()
        .all(|entry| entry.is_table() && name(entry).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{TestCluster, test_cluster};

    /// The cluster under test, written from the test cluster's names: the
    /// first node gives its own service name, tuning and Locations, the
    /// second nothing, the third its Storage nodes.
    struct Written {
        text: String,
        cluster: String,
        first: String,
        second: String,
        third: String,
    }

    fn written(cluster: &TestCluster) -> Written {
        let (first, second, third) = (
            cluster.node(0).name.clone(),
            cluster.node(1).name.clone(),
            cluster.node(2).name.clone(),
        );
        let name = &cluster.name;
        let text = format!(
            r#"
[service]
name = "xmip"
cluster_name = "{name}"

[tuning]
segments = 44
receive_idle = "1m"

[[receive_locations]]
name = "drop"
start = true
transport = "xmip-core-transport-file"
address = "/var/xmip/in"

[nodes.{first}.service]
name = "xmip-{first}"

[nodes.{first}.tuning]
receive_idle = "30s"

[[nodes.{first}.receive_locations]]
name = "drop"
address = "/srv/{first}/in"

[[nodes.{first}.receive_locations]]
name = "api"
start = true
transport = "xmip-core-transport-http"
address = "https://{first}.example/in"

[nodes.{second}]

[nodes.{third}.storage]
nodes = ["{third}.example:7443"]
"#
        );
        Written {
            text,
            cluster: name.clone(),
            first,
            second,
            third,
        }
    }

    fn document(written: &Written, node: &str) -> crate::XmipConfigurationDocument {
        crate::parse_toml(&slice(&written.text, node).expect("slices")).expect("reads")
    }

    #[test]
    fn a_node_reads_the_shared_sections_and_its_own_its_own_winning() {
        let written = written(&test_cluster());
        let first = document(&written, &written.first);
        assert_eq!(first.service.name, format!("xmip-{}", written.first));
        assert_eq!(first.service.cluster_name, written.cluster);
        assert_eq!(first.service.node_name, written.first);
        assert_eq!(first.tuning.0["segments"].as_integer(), Some(44));
        assert_eq!(first.tuning.0["receive_idle"].as_str(), Some("30s"));
        let names: Vec<_> = first.receive_locations.iter().map(|l| &l.name).collect();
        assert_eq!(names, ["drop", "api"]);
        assert_eq!(
            first.receive_locations[0].address,
            format!("/srv/{}/in", written.first)
        );
        assert_eq!(
            first.receive_locations[0].transport,
            "xmip-core-transport-file"
        );
    }

    #[test]
    fn a_node_with_nothing_of_its_own_reads_the_shared_sections() {
        let written = written(&test_cluster());
        let second = document(&written, &written.second);
        assert_eq!(second.service.name, "xmip");
        assert_eq!(second.service.node_name, written.second);
        assert_eq!(second.receive_locations[0].address, "/var/xmip/in");
        assert!(second.storage.nodes.is_empty());
        assert_eq!(
            document(&written, &written.third).storage.nodes[0].as_str(),
            format!("{}.example:7443", written.third)
        );
    }

    #[test]
    fn the_nodes_are_named_in_order_of_name_and_each_slices() {
        let cluster = test_cluster();
        let written = written(&cluster);
        let mut expected = [
            written.first.clone(),
            written.second.clone(),
            written.third.clone(),
        ];
        expected.sort();
        assert_eq!(nodes(&written.text).expect("reads"), expected);
        assert_eq!(slices(&written.text).expect("slices").len(), 3);
        assert!(is_cluster(&written.text));
        assert!(is_cluster(&cluster.text));
        assert!(!is_cluster("[service]\nname = \"n\"\n"));
    }

    #[test]
    fn a_node_the_cluster_does_not_declare_is_refused_naming_those_it_does() {
        let cluster = test_cluster();
        let absent = cluster.absent();
        let refused = slice(&cluster.text, &absent).expect_err("refused");
        assert!(
            refused.contains(&format!("no [nodes.{absent}]")),
            "{refused}"
        );
        for node in &cluster.nodes {
            assert!(
                refused.contains(&format!("[nodes.{}]", node.name)),
                "{refused}"
            );
        }
    }

    #[test]
    fn a_node_name_written_by_hand_is_refused_where_it_disagrees() {
        let cluster = test_cluster();
        let (node, other) = (&cluster.node(0).name, &cluster.node(1).name);
        let shared = format!("[service]\nnode_name = \"{node}\"\n[nodes.{node}]\n");
        assert!(
            slice(&shared, node)
                .expect_err("refused")
                .contains("node_name")
        );
        let disagrees = format!("[nodes.{node}.service]\nnode_name = \"{other}\"\n");
        assert!(
            slice(&disagrees, node)
                .expect_err("refused")
                .contains(&format!("'{other}'"))
        );
        let same = format!(
            "[service]\nname = \"n\"\ncluster_name = \"{}\"\n\
             [nodes.{node}.service]\nnode_name = \"{node}\"\n",
            cluster.name
        );
        assert!(slice(&same, node).is_ok());
    }

    #[test]
    fn an_application_section_is_sliced_to_the_nodes_that_bind_it() {
        let cluster = test_cluster();
        let (first, second) = (&cluster.node(0).name, &cluster.node(1).name);
        let text = format!(
            "[service]\nname = \"xmip\"\ncluster_name = \"{}\"\n\n\
             [[xmip_applications]]\nname = \"Orders\"\n\n\
             [[xmip_applications.send_ports]]\nname = \"Billing\"\n\n\
             [[xmip_applications]]\nname = \"Spare\"\n\n\
             [[nodes.{first}.applications]]\nname = \"Orders\"\n\n[nodes.{second}]\n",
            cluster.name
        );
        let binding = crate::parse_toml(&slice(&text, first).expect("slices")).expect("reads");
        let names: Vec<_> = binding.xmip_applications.iter().map(|s| s.name()).collect();
        assert_eq!(names, ["Orders"]);
        assert!(crate::binding_problems(&binding).is_empty());

        let bare = crate::parse_toml(&slice(&text, second).expect("slices")).expect("reads");
        assert!(bare.xmip_applications.is_empty());
    }

    #[test]
    fn a_slice_is_no_cluster_and_nodes_not_tables_are_refused() {
        let cluster = test_cluster();
        let node = &cluster.node(0).name;
        assert!(!is_cluster(&slice(&cluster.text, node).expect("slices")));
        assert!(nodes(&format!("nodes = [\"{node}\"]\n")).is_err());
        assert!(nodes(&format!("[nodes]\n{node} = 1\n")).is_err());
    }

    #[test]
    fn each_node_declares_the_role_words_its_roles_says_and_none_where_it_says_none() {
        let cluster = test_cluster();
        let [one, two] = [0, 1].map(|place| cluster.node(place).name.clone());
        let text = format!("[nodes.{one}]\n{ROLES} = \"processing, sending\"\n\n[nodes.{two}]\n");
        let mut expected = vec![
            (
                one.clone(),
                vec!["processing".to_string(), "sending".to_string()],
            ),
            (two.clone(), Vec::new()),
        ];
        expected.sort();
        assert_eq!(roles(&text).expect("reads"), expected);
        let plus = format!("[nodes.{one}]\n{ROLES} = \"receiving+sending\"\n");
        assert_eq!(roles(&plus).expect("reads")[0].1, ["receiving", "sending"]);
        assert!(roles(&format!("[nodes.{one}]\n{ROLES} = 1\n")).is_err());
        let declared = roles(&cluster.text).expect("the test cluster reads");
        assert_eq!(declared.len(), cluster.nodes.len());
    }
}
