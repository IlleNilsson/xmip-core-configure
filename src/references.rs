//! What in a cluster's `xmip.toml` names what (ADR-0064, amendment
//! 2026-10-13: go to a definition, find every reference, rename across the
//! file, and refuse or follow a deletion of something still named).
//!
//! A name in one table that stands for an entry declared in another:
//!
//! - an Xmip Application's Receive Location names its `receive_port`, a
//!   Receive Port of the same Application;
//! - a Send Port Group's `send_ports` name Send Ports of its Application;
//! - a Subscription's `destination` names a Work Process, a Send Port or a
//!   Send Port Group of its Application, keyed by the case
//!   (`route::Subscriber`'s own words);
//! - a binding, the cluster's `[[applications]]` or a node's, names the
//!   Application section it binds, and each Receive Location and Send Port
//!   it binds names one the Application declares, and the `node` that takes
//!   it;
//! - a node's own `[service] node_name` names the node;
//! - a Receive Location's `accept.party` names Parties of `[[parties]]`;
//! - a call to a compiled unit names its module and unit, and a Port's or
//!   Location's `prepare`, `transform`, `promote` and `demote` the calls of
//!   its document ([`calls`]).
//!
//! [`references`] answers each with the entry holding it, the name written
//! — or none, where the key may name one and is not written yet — what it
//! names, where that is declared, and every name it may take. The
//! Application's design reads these the same way ([`crate::application`],
//! [`crate::binding`]); this says where each is in the file.

mod calls;

use route::Subscriber;
use serde::{Deserialize, Serialize};
use toml::{Table, Value};

use crate::cluster::NODES;
use crate::field::entry_name;
use crate::section::SECTIONS;

/// One name that stands for an entry declared elsewhere in the file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Reference {
    /// The section of the table holding it, as the views name it.
    pub holder: Vec<String>,
    /// Its key within that table: `["destination", "send-port"]`.
    pub key: Vec<String>,
    /// Its place in an array; none for the array's next element.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    /// The name written; none where the key is not written yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// What it names, as a person reads it: `Send Port`, `node`.
    pub names: &'static str,
    /// The section of what it names, where one of that name is declared.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<Vec<String>>,
    /// Every name it may take: what is declared where it looks.
    pub choices: Vec<String>,
}

/// Every reference in the cluster's file `cluster`.
///
/// # Errors
/// The reader's words when the text is not TOML.
pub fn references(cluster: &str) -> Result<Vec<Reference>, String> {
    let table: Table = cluster
        .parse()
        .map_err(|error| format!("{}: {error}", crate::PARSE_FAILED))?;
    let mut found = Vec::new();
    for (application, own) in named(Some(&table), SECTIONS) {
        designed(&application, own, &mut found);
    }

    let nodes: Vec<(String, &Table)> = table
        .get(NODES)
        .and_then(Value::as_table)
        .into_iter()
        .flatten()
        .filter_map(|(name, own)| own.as_table().map(|own| (name.clone(), own)))
        .collect();
    let node_names: Vec<String> = nodes.iter().map(|(name, _)| name.clone()).collect();
    let applications: Vec<(String, &Table)> = named(Some(&table), SECTIONS).collect();

    let mut holders = vec![(Vec::new(), &table)];
    holders.extend(nodes.iter().map(|(name, own)| (path(&[NODES, name]), *own)));
    for (at, holder) in &holders {
        accepting(at, holder, &table, &mut found);
    }
    calls::calls(&table, &mut found);
    for (at, holder) in holders {
        for (binding, entry) in named(Some(holder), "applications") {
            let section = [at.clone(), path(&["applications", &binding])].concat();
            let declaring = applications.iter().find(|(name, _)| *name == binding);
            bound(
                &section,
                entry,
                declaring,
                &applications,
                &node_names,
                &mut found,
            );
        }
    }

    for (node, own) in &nodes {
        let said = own
            .get("service")
            .and_then(|service| service.get("node_name"));
        if let Some(name) = said.and_then(Value::as_str) {
            let holder = path(&[NODES, node, "service"]);
            found.push(slot(
                holder,
                &["node_name"],
                None,
                Some(name),
                "node",
                &[NODES],
                &node_names,
            ));
        }
    }
    Ok(found)
}

/// The Parties each Receive Location `holder` holds at `at`, its own and
/// each bound one, accepts: of the cluster's `[[parties]]` and the node's.
fn accepting(at: &[String], holder: &Table, cluster: &Table, found: &mut Vec<Reference>) {
    let mut parties: Vec<(Vec<String>, String)> = Vec::new();
    for (scope, table) in [(Vec::new(), cluster), (at.to_vec(), holder)] {
        for (name, _) in named(Some(table), "parties") {
            if !parties.iter().any(|(_, known)| *known == name) {
                parties.push(([scope.clone(), path(&["parties"])].concat(), name));
            }
        }
    }
    let names: Vec<String> = parties.iter().map(|(_, name)| name.clone()).collect();
    let mut locations: Vec<(Vec<String>, &Table)> = named(Some(holder), "receive_locations")
        .map(|(name, own)| {
            (
                [at.to_vec(), path(&["receive_locations", &name])].concat(),
                own,
            )
        })
        .collect();
    for (binding, own) in named(Some(holder), "applications") {
        for (name, bound) in named(Some(own), "receive_locations") {
            let section = path(&["applications", &binding, "receive_locations", &name]);
            locations.push(([at.to_vec(), section].concat(), bound));
        }
    }
    for (section, location) in locations {
        let accepted = location
            .get("accept")
            .and_then(|accept| accept.get("party"))
            .and_then(Value::as_array);
        let key = ["accept", "party"];
        for (index, party) in accepted.into_iter().flatten().enumerate() {
            let name = party.as_str();
            let target = parties
                .iter()
                .find(|(_, known)| Some(known.as_str()) == name)
                .map(|(list, known)| [list.clone(), vec![known.clone()]].concat());
            found.push(Reference {
                holder: section.clone(),
                key: path(&key),
                index: Some(index),
                name: name.map(str::to_string),
                names: "Party",
                target,
                choices: names.clone(),
            });
        }
        found.push(Reference {
            holder: section,
            key: path(&key),
            index: None,
            name: None,
            names: "Party",
            target: None,
            choices: names.clone(),
        });
    }
}

/// The references an Application's own design holds.
fn designed(application: &str, own: &Table, found: &mut Vec<Reference>) {
    let at = |list: &str| path(&[SECTIONS, application, list]);
    let names = |list: &str| {
        named(Some(own), list)
            .map(|(name, _)| name)
            .collect::<Vec<_>>()
    };
    let entry = |list: &str, name: &str| [at(list), vec![name.to_string()]].concat();
    let [ports, sends] = [names("receive_ports"), names("send_ports")];

    for (location, table) in named(Some(own), "receive_locations") {
        let written = table.get("receive_port").and_then(Value::as_str);
        found.push(slot(
            entry("receive_locations", &location),
            &["receive_port"],
            None,
            written,
            "Receive Port",
            &[SECTIONS, application, "receive_ports"],
            &ports,
        ));
    }

    for (group, table) in named(Some(own), "send_port_groups") {
        let holder = entry("send_port_groups", &group);
        let members = table.get("send_ports").and_then(Value::as_array);
        let target = [SECTIONS, application, "send_ports"];
        for (index, member) in members.into_iter().flatten().enumerate() {
            let name = member.as_str();
            let one = slot(
                holder.clone(),
                &["send_ports"],
                Some(index),
                name,
                "Send Port",
                &target,
                &sends,
            );
            found.push(one);
        }
        found.push(slot(
            holder,
            &["send_ports"],
            None,
            None,
            "Send Port",
            &target,
            &sends,
        ));
    }

    for (id, table) in named(Some(own), "subscriptions") {
        let written = table
            .get("destination")
            .and_then(|value| Subscriber::deserialize(value.clone()).ok());
        for case in [
            Subscriber::WorkProcess(String::new()),
            Subscriber::SendPort(String::new()),
            Subscriber::SendGroup(String::new()),
        ] {
            let Some(word) = word_of(&case) else { continue };
            let (names_what, list) = match case {
                Subscriber::WorkProcess(_) => ("Work Process", "work_processes"),
                Subscriber::SendPort(_) => ("Send Port", "send_ports"),
                Subscriber::SendGroup(_) => ("Send Port Group", "send_port_groups"),
            };
            let name = written
                .as_ref()
                .filter(|said| std::mem::discriminant(*said) == std::mem::discriminant(&case))
                .map(Subscriber::name);
            found.push(slot(
                entry("subscriptions", &id),
                &["destination", &word],
                None,
                name,
                names_what,
                &[SECTIONS, application, list],
                &names(list),
            ));
        }
    }
}

/// The references a binding holds: the section it binds, each bound
/// Location's declaration and the node that takes it.
fn bound(
    section: &[String],
    binding: &Table,
    declaring: Option<&(String, &Table)>,
    applications: &[(String, &Table)],
    nodes: &[String],
    found: &mut Vec<Reference>,
) {
    let name = section.last().map(String::as_str);
    let all: Vec<String> = applications.iter().map(|(name, _)| name.clone()).collect();
    found.push(slot(
        section.to_vec(),
        &["name"],
        None,
        name,
        "Xmip Application",
        &[SECTIONS],
        &all,
    ));
    let application = name.unwrap_or_default();

    for (list, what) in [
        ("receive_locations", "Receive Location"),
        ("send_ports", "Send Port"),
    ] {
        let declared: Vec<String> = declaring
            .map(|(_, own)| named(Some(own), list).map(|(name, _)| name).collect())
            .unwrap_or_default();
        for (location, entry) in named(Some(binding), list) {
            let holder = [section.to_vec(), path(&[list, &location])].concat();
            let target = [SECTIONS, application, list];
            found.push(slot(
                holder.clone(),
                &["name"],
                None,
                Some(&location),
                what,
                &target,
                &declared,
            ));
            let node = entry.get("node").and_then(Value::as_str);
            found.push(slot(holder, &["node"], None, node, "node", &[NODES], nodes));
        }
    }
}

/// A reference, resolved against the names declared at `list`.
fn slot(
    holder: Vec<String>,
    key: &[&str],
    index: Option<usize>,
    name: Option<&str>,
    names: &'static str,
    list: &[&str],
    choices: &[String],
) -> Reference {
    let target = name
        .filter(|name| choices.iter().any(|choice| choice == name))
        .map(|name| [path(list), vec![name.to_string()]].concat());
    Reference {
        holder,
        key: path(key),
        index,
        name: name.map(str::to_string),
        names,
        target,
        choices: choices.to_vec(),
    }
}

/// The key a destination's case is written under: its own serialization.
fn word_of(case: &Subscriber) -> Option<String> {
    match Value::try_from(case).ok()? {
        Value::Table(table) => table.keys().next().cloned(),
        _ => None,
    }
}

fn path(steps: &[&str]) -> Vec<String> {
    steps.iter().map(ToString::to_string).collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::tests::written;

    fn at<'r>(found: &'r [Reference], holder: &[&str], key: &[&str]) -> &'r Reference {
        found
            .iter()
            .find(|reference| reference.holder == holder && reference.key == key)
            .unwrap_or_else(|| panic!("no {key:?} in {holder:?}"))
    }

    #[test]
    fn a_destination_a_receive_port_and_a_binding_each_name_their_declaration() {
        let (text, first, _) = written();
        let found = references(&text).expect("reads");

        let billing = ["xmip_applications", "Orders", "subscriptions", "billing"];
        let destination = at(&found, &billing, &["destination", "send-port"]);
        assert_eq!(destination.name.as_deref(), Some("Billing"));
        assert_eq!(
            destination.target,
            Some(path(&[
                "xmip_applications",
                "Orders",
                "send_ports",
                "Billing"
            ]))
        );
        let process = at(&found, &billing, &["destination", "work-process"]);
        assert_eq!(
            (process.name.as_ref(), process.target.as_ref()),
            (None, None)
        );

        let location = [
            "xmip_applications",
            "Orders",
            "receive_locations",
            "OrdersIn",
        ];
        assert_eq!(at(&found, &location, &["receive_port"]).choices, ["Orders"]);

        let binding = at(&found, &["applications", "Orders"], &["name"]);
        assert_eq!(binding.target, Some(path(&["xmip_applications", "Orders"])));
        let bound = ["applications", "Orders", "receive_locations", "OrdersIn"];
        assert_eq!(at(&found, &bound, &["name"]).target, Some(path(&location)));
        let node = at(&found, &bound, &["node"]);
        assert_eq!(node.target, Some(path(&["nodes", &first])));
        assert_eq!(node.names, "node");
    }

    #[test]
    fn a_name_nothing_declares_is_dangling_and_offers_what_is() {
        let (text, _, _) = written();
        let text = text.replace("send-port = \"Billing\"", "send-port = \"Biling\"");
        let found = references(&text).expect("reads");

        let billing = ["xmip_applications", "Orders", "subscriptions", "billing"];
        let dangling = at(&found, &billing, &["destination", "send-port"]);
        assert_eq!(dangling.name.as_deref(), Some("Biling"));
        assert_eq!(dangling.target, None);
        assert_eq!(dangling.choices, ["Billing"]);
    }

    #[test]
    fn an_accepted_party_and_a_calls_module_unit_and_attachments_name_their_declarations() {
        let (text, first, _) = written();
        let text = format!(
            "[[parties]]\nid = \"0192f000-0000-7000-8000-000000000001\"\nname = \"partner\"\n\
             identities = []\n\n\
             [[modules]]\nname = \"orders\"\nstart = true\n[modules.manifest]\n\
             capabilities = []\noffline = true\n[modules.manifest.identity]\nname = \"orders\"\n\
             version = \"1\"\n[modules.manifest.entrypoint]\n\
             [[modules.manifest.units]]\nname = \"Codec\"\ncapability = \"prepare\"\n\
             stages = [\"receive\"]\noffline = true\n\
             [[modules.manifest.units]]\nname = \"Out\"\ncapability = \"prepare\"\n\
             stages = [\"send\"]\noffline = true\n\n\
             [[preparation_steps]]\nname = \"Unzip\"\nmodule = \"orders\"\nunit = \"Codec\"\n\
             [[preparation_steps]]\nname = \"Zip\"\nmodule = \"orders\"\nunit = \"Out\"\n\n\
             {text}\n[[nodes.{first}.receive_locations]]\nname = \"in\"\ntransport = \"t\"\n\
             address = \"a\"\nprepare = [\"Unzip\"]\n\
             [nodes.{first}.receive_locations.accept]\nparty = [\"partner\"]\n"
        );
        let found = references(&text).expect("reads");
        let location = ["nodes", &first, "receive_locations", "in"];

        let party = found
            .iter()
            .find(|r| r.holder == location && r.key == ["accept", "party"] && r.index == Some(0))
            .expect("the accepted Party");
        assert_eq!(party.target, Some(path(&["parties", "partner"])));
        let step = found
            .iter()
            .find(|r| r.holder == location && r.key == ["prepare"] && r.index == Some(0))
            .expect("the attached step");
        assert_eq!(step.target, Some(path(&["preparation_steps", "Unzip"])));
        assert_eq!(step.choices, ["Unzip"], "Zip serves send alone");
        let unit = at(&found, &["preparation_steps", "Unzip"], &["unit"]);
        assert_eq!(unit.choices, ["Codec", "Out"]);
        assert_eq!(
            unit.target,
            Some(path(&["modules", "orders", "manifest", "units", "Codec"]))
        );
        let module = at(&found, &["preparation_steps", "Zip"], &["module"]);
        assert_eq!(module.target, Some(path(&["modules", "orders"])));
    }
}
