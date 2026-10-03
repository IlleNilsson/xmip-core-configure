//! The test cluster, the one place a test takes a cluster's or a node's
//! name from (the owner, 2026-10-03: names in code and tests are parameters
//! and configuration, never literals).
//!
//! It reads the cluster's `xmip.toml` the variable [`VARIABLE`] names, a
//! path — `Start-XmipTest` sets it — and otherwise the estate's
//! `test/xmip.toml`, through [`crate::cluster`], the one reading of a
//! cluster's file. A test finds a node by what it declares
//! ([`TestCluster::with_role`]) or by its place ([`TestCluster::node`]),
//! and builds the text it tests from the names it is given. A test of two
//! clusters takes the second from its own file, [`OTHER_FILE`]
//! ([`other_cluster`], [`TestCluster::other`]).
//!
//! Behind the `test-support` feature, so never in a production build.

use std::path::{Path, PathBuf};

use crate::cluster;

/// The variable naming the test cluster's file.
pub const VARIABLE: &str = "XMIP_TEST_CLUSTER";

/// Where the estate keeps its test cluster's file, from the estate's root.
pub const ESTATE_FILE: &str = "test/xmip.toml";

/// Where the estate keeps its second test cluster's file, from the
/// estate's root: the cluster beside the test cluster.
pub const OTHER_FILE: &str = "test/other/xmip.toml";

/// The test cluster, read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestCluster {
    /// Where it was read from.
    pub path: PathBuf,
    /// What the file says.
    pub text: String,
    /// Its `[service] cluster_name`.
    pub name: String,
    /// Its nodes, in the order of their names.
    pub nodes: Vec<TestNode>,
}

/// One node of the test cluster.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestNode {
    /// Its name, its key under `[nodes]`.
    pub name: String,
    /// The role words its `roles` declares, as written.
    pub roles: Vec<String>,
}

/// The test cluster: the file [`VARIABLE`] names, else the estate's.
///
/// # Panics
/// Where neither is found or the file does not read as a cluster with a
/// `cluster_name` and one node at least: a test cannot run without it.
#[must_use]
pub fn test_cluster() -> TestCluster {
    read(std::env::var_os(VARIABLE).map_or_else(|| estate_file(ESTATE_FILE), PathBuf::from))
}

/// The second test cluster: the estate's [`OTHER_FILE`], the cluster a test
/// of two takes beside [`test_cluster`].
///
/// # Panics
/// As [`test_cluster`].
#[must_use]
pub fn other_cluster() -> TestCluster {
    read(estate_file(OTHER_FILE))
}

/// The cluster the file at `path` declares.
fn read(path: PathBuf) -> TestCluster {
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the test cluster {}: {error}", path.display()));
    let table: toml::Table = text
        .parse()
        .unwrap_or_else(|error| panic!("the test cluster {}: {error}", path.display()));
    let name = table
        .get("service")
        .and_then(|service| service.get("cluster_name"))
        .and_then(toml::Value::as_str)
        .unwrap_or_else(|| panic!("the test cluster {} names no cluster_name", path.display()))
        .to_string();
    let nodes: Vec<TestNode> = cluster::roles(&text)
        .unwrap_or_else(|problem| panic!("the test cluster {}: {problem}", path.display()))
        .into_iter()
        .map(|(name, roles)| TestNode { name, roles })
        .collect();
    assert!(
        !nodes.is_empty(),
        "the test cluster {} declares no node",
        path.display()
    );
    TestCluster {
        path,
        text,
        name,
        nodes,
    }
}

/// The estate's `file`, found above this crate or the directory the test
/// runs in.
fn estate_file(file: &str) -> PathBuf {
    let here = std::env::current_dir().unwrap_or_default();
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .chain(here.ancestors())
        .map(|directory| directory.join(file))
        .find(|found| found.is_file())
        .unwrap_or_else(|| panic!("no {file} above this crate or this directory"))
}

impl TestCluster {
    /// The node at `place`, in the order of their names.
    ///
    /// # Panics
    /// Where the cluster has fewer nodes: the test needs more than the
    /// cluster gives it.
    #[must_use]
    pub fn node(&self, place: usize) -> &TestNode {
        self.nodes.get(place).unwrap_or_else(|| {
            panic!(
                "the test cluster {} has {} nodes, and a test needs {}",
                self.path.display(),
                self.nodes.len(),
                place + 1
            )
        })
    }

    /// The first node declaring `role`.
    ///
    /// # Panics
    /// Where none does.
    #[must_use]
    pub fn with_role(&self, role: &str) -> &TestNode {
        self.nodes
            .iter()
            .find(|node| node.roles.iter().any(|declared| declared == role))
            .unwrap_or_else(|| {
                panic!(
                    "the test cluster {} has no {role} node",
                    self.path.display()
                )
            })
    }

    /// A name no node of the cluster has: what a test refusing an unknown
    /// node asks for.
    #[must_use]
    pub fn absent(&self) -> String {
        let mut name = format!("{}-absent", self.node(0).name);
        while self.nodes.iter().any(|node| node.name == name) {
            name.push('-');
        }
        name
    }

    /// A cluster's name other than this one's: the second test cluster's
    /// ([`other_cluster`]), what a test of two clusters takes for the
    /// second.
    ///
    /// # Panics
    /// Where the second test cluster is not found, or is named as this one
    /// is: the two files must name two clusters.
    #[must_use]
    pub fn other(&self) -> String {
        let other = other_cluster();
        assert_ne!(
            other.name,
            self.name,
            "{} and {} name one cluster",
            other.path.display(),
            self.path.display()
        );
        other.name
    }

    /// The cluster's scope, `xmip:///<cluster>`.
    #[must_use]
    pub fn scope(&self) -> String {
        format!("{SCOPE_ROOT}{}", self.name)
    }

    /// The scope of the node at `place`, `xmip:///<cluster>/node/<node>`.
    ///
    /// # Panics
    /// As [`TestCluster::node`].
    #[must_use]
    pub fn node_scope(&self, place: usize) -> String {
        format!("{}/node/{}", self.scope(), self.node(place).name)
    }
}

/// Where every scope begins (ADR-0018).
const SCOPE_ROOT: &str = "xmip:///";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_test_cluster_reads_with_a_name_and_its_nodes() {
        let cluster = test_cluster();
        assert!(!cluster.name.is_empty());
        assert!(!cluster.nodes.is_empty());
        let first = cluster.node(0);
        assert!(first.roles.iter().all(|role| !role.is_empty()));
        if let Some(role) = first.roles.first() {
            assert_eq!(cluster.with_role(role), first);
        }
        assert!(
            cluster
                .nodes
                .iter()
                .all(|node| node.name != cluster.absent())
        );
        assert_ne!(cluster.other(), cluster.name);
        let other = other_cluster();
        assert_eq!(cluster.other(), other.name);
        assert!(!other.nodes.is_empty());
        assert_eq!(
            cluster.node_scope(0),
            format!("xmip:///{}/node/{}", cluster.name, first.name)
        );
    }
}
