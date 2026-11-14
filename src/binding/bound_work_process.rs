//! A Work Process an Xmip Application declares, bound to a node that runs
//! it (ADR-0056 and ADR-0066, amendments 2026-11-13): placed by its stage
//! on every node serving the process stage (`crate::placed`), each binding
//! naming it and one such node, the nodes sharing its turns through Xmip
//! Storage. A node running it takes its design's flow — the `module` and
//! `unit` it calls and what the call gives — and starts it.
//!
//! ```toml
//! [[applications.work_processes]]
//! name = "Purchase order confirmation"
//! node = "<node>"
//! ```

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::WorkProcessConfiguration;
use crate::port::DesignedWorkProcess;

/// `[[applications.work_processes]]`: a Work Process the Application
/// declares, and a node that runs it — bound once for each such node. A
/// key neither reads is refused, never ignored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundWorkProcess {
    /// The Work Process, its design's `name`.
    pub name: String,
    /// The node that runs it, a `service.node_name`.
    pub node: String,
}

/// What the Work Processes `bound` of the Application `application` say
/// wrong on their own: each a name and a node, and none bound twice to one
/// node.
pub(super) fn problems(application: &str, bound: &[BoundWorkProcess], problems: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    for one in bound {
        let name = &one.name;
        if name.trim().is_empty() {
            problems.push(format!(
                "a bound Work Process of '{application}' requires a name"
            ));
        } else if !seen.insert((name.as_str(), one.node.as_str())) {
            problems.push(format!(
                "'{application}' binds the Work Process '{name}' to the node '{}' twice",
                one.node
            ));
        }
        if one.node.trim().is_empty() {
            problems.push(format!(
                "the Work Process '{name}' of '{application}' requires the node that runs it"
            ));
        }
    }
}

/// What a node takes of an Application's bound Work Processes: those
/// `bound` gives `node`, each as its design in `declared` says it, started;
/// and the names bound that the Application does not declare.
pub(super) fn taken(
    bound: &[BoundWorkProcess],
    declared: &[DesignedWorkProcess],
    node: &str,
) -> (Vec<WorkProcessConfiguration>, Vec<String>) {
    let mut taken = Vec::new();
    let mut undeclared = Vec::new();
    for one in bound {
        match declared.iter().find(|design| design.name == one.name) {
            Some(design) if one.node == node => taken.push(WorkProcessConfiguration {
                name: design.name.clone(),
                start: true,
                required_modules: Vec::new(),
                extensions: Vec::new(),
                module: design.module.clone(),
                unit: design.unit.clone(),
                settings: design.settings.clone(),
            }),
            Some(_) => {}
            None => undeclared.push(one.name.clone()),
        }
    }
    (taken, undeclared)
}
