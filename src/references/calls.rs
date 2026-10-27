//! What names what among the calls to compiled units (ADR-0066, amendment
//! 2026-10-22): a step entry's or a Work Process's `module` names a
//! `[[modules]]` entry and its `unit` one of that module's units; a Port's
//! or Location's `prepare`, `transform`, `promote`, `demote` and `debatch` name the
//! entries of its own document's lists. A unit is offered only where it
//! fits: a call's `unit` the units of the list's capability, an attachment
//! the entries whose unit serves the stage it is attached at.

use abi::UnitCapability;
use node::Stage;
use toml::{Table, Value};

use super::{Reference, named, path};
use crate::ModuleConfiguration;
use crate::cluster::NODES;
use crate::section::SECTIONS;
use crate::step::{ATTACHMENTS, LISTS};

/// The modules the file declares, with the list each is in.
type Modules = Vec<(Vec<String>, ModuleConfiguration)>;

/// Every reference among the calls in the cluster's file `table`.
pub(super) fn calls(table: &Table, found: &mut Vec<Reference>) {
    let modules = crate::modules::declared_in(table);
    let nodes: Vec<(Vec<String>, &Table)> = table
        .get(NODES)
        .and_then(Value::as_table)
        .into_iter()
        .flatten()
        .filter_map(|(name, own)| own.as_table().map(|own| (path(&[NODES, name]), own)))
        .collect();

    // A node's document: the cluster's own lists and the node's, merged.
    let mut documents = vec![(Vec::new(), vec![(Vec::new(), table)])];
    for (at, own) in &nodes {
        documents.push((at.clone(), vec![(Vec::new(), table), (at.clone(), *own)]));
    }
    for (at, scope) in &documents {
        let holder = scope.last().map_or(table, |(_, own)| *own);
        called(at, holder, &modules, found);
        let mut artifacts = Vec::new();
        for (list, side) in [
            ("receive_locations", Stage::Receive),
            ("send_locations", Stage::Send),
        ] {
            for (name, own) in named(Some(holder), list) {
                artifacts.push(([at.clone(), path(&[list, &name])].concat(), own, side));
            }
        }
        for (binding, own) in named(Some(holder), "applications") {
            for (list, side) in [
                ("receive_locations", Stage::Receive),
                ("send_ports", Stage::Send),
            ] {
                for (name, bound) in named(Some(own), list) {
                    let section = [at.clone(), path(&["applications", &binding, list, &name])];
                    artifacts.push((section.concat(), bound, side));
                }
            }
        }
        for (section, own, side) in artifacts {
            attached(&section, own, side, scope, &modules, found);
        }
    }

    for (application, own) in named(Some(table), SECTIONS) {
        let at = path(&[SECTIONS, &application]);
        called(&at, own, &modules, found);
        let scope = [(at.clone(), own)];
        for (list, side) in [
            ("receive_ports", Stage::Receive),
            ("receive_locations", Stage::Receive),
            ("send_ports", Stage::Send),
        ] {
            for (name, artifact) in named(Some(own), list) {
                let section = [at.clone(), path(&[list, &name])].concat();
                attached(&section, artifact, side, &scope, &modules, found);
            }
        }
    }
}

/// The `module` and `unit` of each step entry and Work Process `holder`
/// holds at `at`.
fn called(at: &[String], holder: &Table, modules: &Modules, found: &mut Vec<Reference>) {
    let lists = LISTS
        .iter()
        .map(|(list, capability, _)| (*list, *capability))
        .chain([("work_processes", UnitCapability::WorkProcess)]);
    for (list, capability) in lists {
        for (name, entry) in named(Some(holder), list) {
            let section = [at.to_vec(), path(&[list, &name])].concat();
            let module = entry.get("module").and_then(Value::as_str);
            let names: Vec<String> = modules.iter().map(|(_, m)| m.name.clone()).collect();
            let declaring =
                module.and_then(|module| modules.iter().find(|(_, m)| m.name == module));
            found.push(reference(
                section.clone(),
                "module",
                None,
                module,
                "Module",
                declaring.map(|(list, m)| [list.clone(), vec![m.name.clone()]].concat()),
                names,
            ));
            let unit = entry.get("unit").and_then(Value::as_str);
            let units: Vec<String> = declaring
                .map(|(_, m)| {
                    let fit = m
                        .manifest
                        .units
                        .iter()
                        .filter(|u| u.capability == capability);
                    fit.map(|u| u.name.clone()).collect()
                })
                .unwrap_or_default();
            let target = declaring
                .filter(|_| unit.is_some_and(|unit| units.iter().any(|u| u == unit)))
                .map(|(list, m)| {
                    let units = path(&["manifest", "units", unit.unwrap_or_default()]);
                    [list.clone(), vec![m.name.clone()], units].concat()
                });
            found.push(reference(
                section, "unit", None, unit, "unit", target, units,
            ));
        }
    }
}

/// What an artifact attaches, against the lists of `scope`, the tables its
/// document merges: each name, where it is declared, and the entries whose
/// unit serves `side`.
fn attached(
    section: &[String],
    artifact: &Table,
    side: Stage,
    scope: &[(Vec<String>, &Table)],
    modules: &Modules,
    found: &mut Vec<Reference>,
) {
    for (key, (list, _, person)) in ATTACHMENTS.into_iter().zip(LISTS) {
        let declared: Vec<(Vec<String>, String, bool)> = scope
            .iter()
            .flat_map(|(at, holder)| {
                named(Some(*holder), list).map(move |(name, entry)| {
                    let serves = serves(entry, side, modules);
                    ([at.clone(), path(&[list])].concat(), name, serves)
                })
            })
            .collect();
        let choices: Vec<String> = declared
            .iter()
            .filter(|(_, _, serves)| *serves)
            .map(|(_, name, _)| name.clone())
            .collect();
        let target = |name: Option<&str>| {
            let name = name?;
            declared
                .iter()
                .find(|(_, declared, _)| declared == name)
                .map(|(list, name, _)| [list.clone(), vec![name.clone()]].concat())
        };
        let written = artifact.get(key);
        if key == "prepare" {
            let steps = written.and_then(Value::as_array).into_iter().flatten();
            for (index, step) in steps.enumerate() {
                let name = step.as_str();
                let one = reference(
                    section.to_vec(),
                    key,
                    Some(index),
                    name,
                    person,
                    target(name),
                    choices.clone(),
                );
                found.push(one);
            }
            found.push(reference(
                section.to_vec(),
                key,
                None,
                None,
                person,
                None,
                choices,
            ));
        } else {
            let name = written.and_then(Value::as_str);
            let one = reference(
                section.to_vec(),
                key,
                None,
                name,
                person,
                target(name),
                choices,
            );
            found.push(one);
        }
    }
}

/// Whether the unit `entry` calls serves `side`.
fn serves(entry: &Table, side: Stage, modules: &Modules) -> bool {
    let (Some(module), Some(unit)) = (
        entry.get("module").and_then(Value::as_str),
        entry.get("unit").and_then(Value::as_str),
    ) else {
        return false;
    };
    modules
        .iter()
        .find(|(_, declared)| declared.name == module)
        .and_then(|(_, declared)| declared.unit(unit))
        .is_some_and(|unit| crate::modules::serves(unit, side))
}

fn reference(
    holder: Vec<String>,
    key: &str,
    index: Option<usize>,
    name: Option<&str>,
    names: &'static str,
    target: Option<Vec<String>>,
    choices: Vec<String>,
) -> Reference {
    Reference {
        holder,
        key: path(&[key]),
        index,
        name: name.map(str::to_string),
        names,
        target,
        choices,
    }
}
