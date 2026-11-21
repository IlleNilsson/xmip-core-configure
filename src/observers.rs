//! The nodes that gather the cluster's view, listed as Observers on every
//! node's operate listener as its slice is written (ADR-0009 and ADR-0027
//! decision 8, amendments 2026-11-13; the owner, 2026-11-12, agreeing:
//! *observers listed by role when the environment is written, not granted
//! at run time*). A node that declares the monitoring role
//! (`node::Capability::gathers`) reads every member's publication over its
//! operate listener; the listener is closed by default (ADR-0009, amendment
//! 2026-11-07), so each member names it, by the subject its node
//! certificate carries, `CN=<node>`, as the cluster's authority issues it
//! (`provision::node`). Written here, in the one slice every environment
//! writes a node's configuration with — the Essentials' served runs and
//! `xmip-service --slice` the deploy templates call — and decided by the
//! roles a node declares, never by its name.

use toml::{Table, Value};

/// The lists a name may hold a role on, each judged with the others.
const LISTS: [&str; 3] = ["observers", "operators", "developers"];

/// Each node `nodes` — a cluster file's `[nodes]` — declares that gathers
/// the cluster, by the name its node certificate answers to, in the order
/// a cluster file's table keeps its keys. A node whose roles do not read is
/// passed over: its own slice refuses them.
#[must_use]
pub fn gathering(nodes: &Table) -> Vec<String> {
    nodes
        .iter()
        .filter(|(_, own)| {
            own.get("roles")
                .and_then(Value::as_str)
                .and_then(|said| node::Capability::parse(said).ok())
                .is_some_and(|declared| declared.gathers())
        })
        .map(|(name, _)| format!("CN={name}"))
        .collect()
}

/// `slice`, a node's configuration or a cluster's file, with each of
/// `names` listed under `[operate]`'s `list` — `observers`, `operators` or
/// `developers` — where none of its lists names it already: compared
/// without regard to case, as a name listed twice is judged
/// (`OperateConfiguration::grants`), so a name the file grants more keeps
/// that role.
///
/// # Errors
/// `[operate]` that is no table, or `list` that is no list.
pub fn listed(slice: &mut Table, list: &str, names: &[String]) -> Result<(), String> {
    if names.is_empty() {
        return Ok(());
    }
    let Value::Table(operate) = slice
        .entry("operate")
        .or_insert_with(|| Value::Table(Table::new()))
    else {
        return Err("[operate] is not a table".to_string());
    };
    let named = |name: &str| {
        LISTS.iter().any(|list| {
            operate
                .get(*list)
                .and_then(Value::as_array)
                .is_some_and(|names| {
                    names
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|held| held.trim().eq_ignore_ascii_case(name))
                })
        })
    };
    let adding: Vec<Value> = names
        .iter()
        .filter(|name| !named(name))
        .map(|name| Value::String(name.clone()))
        .collect();
    let Value::Array(held) = operate
        .entry(list)
        .or_insert_with(|| Value::Array(Vec::new()))
    else {
        return Err(format!("[operate] {list} is a list of names"));
    };
    held.extend(adding);
    Ok(())
}

/// The cluster's file `cluster` with each of `operators` listed under its
/// `[operate] operators`, which every node's slice is then given: on
/// machines of their own the environment names who operates its nodes
/// (`xmip-service --slice --operators`), a certificate's subject,
/// `CN=<person>`, or an account, as the Essentials' run lists its tester on
/// one computer (ADR-0009, amendment 2026-11-07).
///
/// # Errors
/// The file is not TOML, or its `[operate]` is no table or `operators` no
/// list.
pub fn operated_by(cluster: &str, operators: &[String]) -> Result<String, String> {
    if operators.is_empty() {
        return Ok(cluster.to_string());
    }
    let mut table: Table = cluster.parse().map_err(|error| format!("{error}"))?;
    listed(&mut table, "operators", operators)?;
    toml::to_string(&table).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::test_cluster;

    /// The test cluster's node at `place`, by its name: what a node is
    /// called is the file's, never the test's.
    fn name(place: usize) -> String {
        test_cluster().node(place).name.clone()
    }

    /// `[nodes]` declaring each of `declared`, its roles as given: the test
    /// cluster's names, given roles here whatever the file gives them.
    fn nodes(declared: &[(String, &str)]) -> Table {
        declared
            .iter()
            .map(|(name, roles)| {
                let own = Table::from_iter([("roles".to_string(), Value::from(*roles))]);
                (name.clone(), Value::Table(own))
            })
            .collect()
    }

    #[test]
    fn the_nodes_declaring_monitoring_gather_whatever_they_are_called() {
        // The test cluster's names given other roles than the file gives
        // them: only the roles decide, never the name.
        let declared = nodes(&[
            (name(0), "operational, monitoring"),
            (name(1), "receiving"),
            (name(2), "executing+monitoring"),
            (name(3), "no such role"),
        ]);
        let mut expected = vec![format!("CN={}", name(0)), format!("CN={}", name(2))];
        expected.sort();
        assert_eq!(gathering(&declared), expected);
        // The same names with the roles swapped swap what gathers.
        let swapped = nodes(&[(name(0), "receiving"), (name(1), "monitoring")]);
        assert_eq!(gathering(&swapped), [format!("CN={}", name(1))]);
    }

    #[test]
    fn every_node_sliced_from_the_test_cluster_lists_the_nodes_declaring_monitoring() {
        let cluster = test_cluster();
        let watching: Vec<String> = cluster
            .nodes
            .iter()
            .filter(|one| {
                node::Capability::parse(&one.roles.join(",")).is_ok_and(|it| it.gathers())
            })
            .map(|one| format!("CN={}", one.name))
            .collect();
        assert!(!watching.is_empty(), "the test cluster declares one");
        for one in &cluster.nodes {
            let sliced = crate::cluster::slice(&cluster.text, &one.name).expect("slices");
            assert!(crate::parse_toml(&sliced).is_ok(), "{} reads", one.name);
            let table: Table = sliced.parse().expect("TOML");
            let observers = table["operate"]["observers"].as_array().expect("observers");
            for name in &watching {
                assert!(
                    observers.iter().any(|held| held.as_str() == Some(name)),
                    "{} lists {name}",
                    one.name
                );
            }
        }
    }

    #[test]
    fn every_gathering_node_is_listed_once_and_a_name_granted_more_keeps_it() {
        let (first, second, third) = (name(0), name(1), name(2));
        let mut slice = Table::new();
        let operate = Table::from_iter([
            (
                "operators".to_string(),
                Value::from(vec![format!("cn={}", second.to_uppercase())]),
            ),
            (
                "observers".to_string(),
                Value::from(vec![format!("CN={first}")]),
            ),
        ]);
        slice.insert("operate".to_string(), Value::Table(operate));
        let gathering = [first, second, third.clone()].map(|one| format!("CN={one}"));
        listed(&mut slice, "observers", &gathering).expect("listed");
        let observers = slice["operate"]["observers"].as_array().expect("observers");
        let names: Vec<&str> = observers.iter().filter_map(Value::as_str).collect();
        let added = format!("CN={third}");
        assert_eq!(names, [gathering[0].as_str(), added.as_str()]);
        // A slice with no [operate] is given one; none gathering, nothing.
        let mut bare = Table::new();
        listed(&mut bare, "observers", &[]).expect("nothing");
        assert!(bare.is_empty());
        listed(&mut bare, "observers", &gathering).expect("listed");
        assert_eq!(
            bare["operate"]["observers"].as_array().map(Vec::len),
            Some(3)
        );
    }

    #[test]
    fn every_node_sliced_lists_the_operators_the_environment_names() {
        let cluster = test_cluster();
        let operators = ["CN=operator".to_string(), r"HOST\installer".to_string()];
        let operated = operated_by(&cluster.text, &operators).expect("listed");
        for one in &cluster.nodes {
            let sliced = crate::cluster::slice(&operated, &one.name).expect("slices");
            let table: Table = sliced.parse().expect("TOML");
            let held = table["operate"]["operators"].as_array().expect("operators");
            for name in &operators {
                assert!(
                    held.iter().any(|one| one.as_str() == Some(name.as_str())),
                    "{} lists {name}",
                    one.name
                );
            }
        }
        assert_eq!(operated_by(&cluster.text, &[]).expect("same"), cluster.text);
    }
}
