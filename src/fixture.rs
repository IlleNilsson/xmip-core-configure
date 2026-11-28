//! The test cluster, the one place a test takes a cluster's or a node's
//! name from (the owner, 2026-10-03: names in code and tests are parameters
//! and configuration, never literals).
//!
//! It is the Reference Environment, [`REFERENCE_FILE`] (the owner,
//! 2026-11-28, naming it by its short name: it *should evolve for all
//! tests*; ADR-0028, amendment 2026-11-28), read through
//! [`crate::cluster`], the one reading of a cluster's file, and named as its
//! file names it ([`crate::cluster::cluster_name`]). A test finds a node by
//! what it declares ([`TestCluster::with_role`]) or by its place
//! ([`TestCluster::node`]), and builds the text it tests from the names it
//! is given. A test of two clusters takes the second's name from
//! [`TestCluster::other`], derived from the first's; no file is kept for
//! it. Where a test needs what the Reference Environment lacks, the
//! Reference Environment grows.
//!
//! Behind the `test-support` feature, so never in a production build.

use std::path::{Path, PathBuf};

use crate::cluster;

/// Where the estate keeps the Reference Environment, the test cluster's
/// file, from the estate's root.
pub const REFERENCE_FILE: &str =
    "test/core/system-test/configuration/Xmip-reference-environment.xmip";

/// The test cluster, read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestCluster {
    /// Where it was read from.
    pub path: PathBuf,
    /// What the file says.
    pub text: String,
    /// The cluster's name its file declares ([`cluster::cluster_name`]).
    pub name: String,
    /// Its nodes, in the order of their names.
    pub nodes: Vec<TestNode>,
}

/// One node of the test cluster.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestNode {
    /// Its name, its key under `[nodes]`.
    pub name: String,
    /// The words of the roles its `roles` declares, as one declaration says
    /// them (`node::NodeRole::declared`).
    pub roles: Vec<String>,
}

/// The test cluster: the estate's Reference Environment.
///
/// # Panics
/// Where it is not found or does not read as a cluster with a name and one
/// node at least: a test cannot run without it.
#[must_use]
pub fn test_cluster() -> TestCluster {
    read(estate_file(REFERENCE_FILE))
}

/// The cluster the file at `path` declares.
fn read(path: PathBuf) -> TestCluster {
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the test cluster {}: {error}", path.display()));
    let name = cluster::cluster_name(&text)
        .unwrap_or_else(|problem| panic!("the test cluster {}: {problem}", path.display()))
        .unwrap_or_else(|| panic!("the test cluster {} names no cluster", path.display()));
    let mut nodes: Vec<TestNode> = cluster::roles(&text)
        .unwrap_or_else(|problem| panic!("the test cluster {}: {problem}", path.display()))
        .into_iter()
        .map(|(name, roles)| TestNode {
            name,
            roles: roles.iter().map(|role| role.name().to_string()).collect(),
        })
        .collect();
    // The places every test counts its nodes by: the order of their names,
    // whatever order the file writes them in, which is placement's.
    nodes.sort_by(|one, another| one.name.cmp(&another.name));
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

    /// A cluster's name other than this one's, what a test of two clusters
    /// takes for the second: this one's with `-other`, a second cluster
    /// derived from this one and kept in no file of its own.
    #[must_use]
    pub fn other(&self) -> String {
        format!("{}-other", self.name)
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
        assert_ne!(cluster.name, "");
        assert!(
            cluster.path.ends_with(REFERENCE_FILE),
            "{}",
            cluster.path.display()
        );
        assert_eq!(
            cluster::cluster_name(&cluster.text).expect("reads"),
            Some(cluster.name.clone())
        );
        assert_ne!(cluster.nodes, [] as [TestNode; 0]);
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
        assert_eq!(
            cluster.other(),
            test_cluster().other(),
            "derived, so the same"
        );
        assert_eq!(
            cluster.node_scope(0),
            format!("xmip:///{}/node/{}", cluster.name, first.name)
        );
    }
}
