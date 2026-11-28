//! The cluster's name its file declares, and the slice made as that
//! cluster: the file names the cluster (the owner, 2026-11-28: *CT should
//! not exist*; ADR-0028, amendment 2026-11-28).

use toml::{Table, Value};

use super::{read, sliced};

/// The cluster's name its file `cluster` declares: its `[service]
/// cluster_name`, or, where it declares none, its `[service] short_name`
/// ([`crate::ShortName`]). Every reader asks this one rule: the slicing,
/// the runtime's signing, the operator surfaces through
/// `xmip_cluster_views_v1` and every test's fixture. None where the file
/// declares neither, and the environment names it ([`slice_named`]).
///
/// # Errors
/// When the text is not TOML, or its short name is no [`crate::ShortName`].
pub fn cluster_name(cluster: &str) -> Result<Option<String>, String> {
    let table = read(cluster)?;
    let written = table
        .get("service")
        .and_then(|service| service.get("cluster_name"))
        .and_then(Value::as_str);
    match written {
        Some(name) => Ok(Some(name.to_string())),
        None => Ok(crate::service::short_name(cluster)?.map(String::from)),
    }
}

/// The node `node`'s configuration document, sliced from the cluster's file
/// `cluster` as the cluster `name`. The file names its cluster
/// ([`cluster_name`]), and a file naming another than `name` is refused,
/// naming both; a file declaring no name leaves it to the environment it
/// runs in, `xmip-service --cluster` on a machine. The slice's `[service]
/// cluster_name` is `name`, and its service is `xmip-<node>` where nothing
/// names it (ADR-0053).
///
/// # Errors
/// As [`super::slice`]; where `name` is empty, or the file names another
/// cluster.
pub fn slice_named(cluster: &str, node: &str, name: &str) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("the environment names no cluster for cluster.xmip".to_string());
    }
    if let Some(declared) = cluster_name(cluster)?.filter(|declared| declared != name) {
        return Err(format!(
            "cluster.xmip names the cluster '{declared}', and its environment names '{name}'"
        ));
    }
    let mut table = read(cluster)?;
    let Value::Table(service) = table
        .entry("service")
        .or_insert_with(|| Value::Table(Table::new()))
    else {
        return Err("[service] is not a table".to_string());
    };
    service
        .entry("cluster_name")
        .or_insert_with(|| Value::String(name.to_string()));
    service
        .entry("name")
        .or_insert_with(|| Value::String(format!("xmip-{node}")));
    let named = toml::to_string(&table).map_err(|error| error.to_string())?;
    sliced(&named, node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::ROLES;
    use crate::fixture::test_cluster;

    /// A file names its cluster by its short name where it declares no
    /// `cluster_name`, and a slice for another cluster is refused; the
    /// slice says the name in `cluster_name` and keeps the short name.
    #[test]
    fn a_file_declaring_its_short_name_names_its_cluster_by_it() {
        let cluster = test_cluster();
        let node = &cluster.node(0).name;
        let short = format!("[service]\nshort_name = \"{}\"\n", cluster.name);
        let text = format!("{short}[nodes.{node}]\n{ROLES} = \"storage\"\n");
        assert_eq!(
            cluster_name(&text).expect("reads"),
            Some(cluster.name.clone())
        );
        let document = crate::parse_toml(&slice_named(&text, node, &cluster.name).expect("slices"))
            .expect("reads as a node's");
        assert_eq!(document.service.cluster_name, cluster.name);
        assert_eq!(
            document.service.short_name.map(String::from),
            Some(cluster.name.clone())
        );
        let other = cluster.other();
        let refused = slice_named(&text, node, &other).expect_err("another cluster");
        assert!(
            refused.contains(&format!("'{}'", cluster.name)) && refused.contains(&other),
            "{refused}"
        );
        let both = text.replace(
            "[service]\n",
            &format!("[service]\ncluster_name = \"{other}\"\n"),
        );
        assert_eq!(
            cluster_name(&both).expect("reads"),
            Some(other),
            "cluster_name first"
        );
        assert!(cluster_name("[service]\nshort_name = \"a b\"\n").is_err());
    }

    /// A cluster's file that declares no name leaves it to the environment
    /// it runs in, and a file naming another is refused.
    #[test]
    fn the_environment_names_a_cluster_its_file_does_not() {
        let cluster = test_cluster();
        let (name, other) = (&cluster.name, &cluster.other());
        let node = &cluster.node(0).name;
        let unnamed = format!("[nodes.{node}]\n{ROLES} = \"storage\"\n");

        assert_eq!(cluster_name(&unnamed).expect("reads"), None);
        let sliced = slice_named(&unnamed, node, name).expect("slices");
        let document = crate::parse_toml(&sliced).expect("reads as a node's");
        assert_eq!(document.service.cluster_name, *name);
        assert_eq!(document.service.name, format!("xmip-{node}"));
        assert_eq!(document.service.node_name, *node);

        let written = format!("[service]\nname = \"xmip\"\ncluster_name = \"{name}\"\n{unnamed}");
        assert_eq!(
            cluster_name(&written).expect("reads").as_deref(),
            Some(name.as_str())
        );
        let same = crate::parse_toml(&slice_named(&written, node, name).expect("the same name"))
            .expect("reads");
        assert_eq!(same.service.name, "xmip", "a name the file gives is kept");
        let refused = slice_named(&written, node, other).expect_err("another cluster");
        assert!(
            refused.contains(&format!("'{name}'")) && refused.contains(&format!("'{other}'")),
            "{refused}"
        );
        assert!(
            slice_named(&unnamed, node, " ").is_err(),
            "no name is no cluster"
        );
    }
}
