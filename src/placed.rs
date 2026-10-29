//! Where a cluster's Xmip Applications run: each Receive Location and Send
//! Port the cluster's file designs, placed on a node an environment has —
//! the one placement for every environment that writes no bindings by hand
//! (ADR-0028, amendments 2026-10-21 and 2026-10-22). Each Application's
//! Receive Locations go round robin, in the file's order, over the nodes
//! present that declare `receiving` or `executing`; its Send Ports over
//! those declaring `sending` or `executing`.
//!
//! The Playground places the nodes it starts on one computer by it, over
//! folders of its run; `xmip-service --slice --bind` places a node on a
//! machine of its own by it, over the Parties' far end the Playground
//! serves ([`far_end`]); and the Playground drives what it placed, so the
//! two can never place differently.

use crate::{cluster, section};

/// The port the Parties' far end is served on where an address names none.
pub const FAR_END_PORT: u16 = 7422;

/// The two lists of an Application a placement is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placed {
    /// A Receive Location.
    Receive,
    /// A Send Port.
    Send,
}

impl Placed {
    /// The list's key in a binding.
    #[must_use]
    pub const fn list(self) -> &'static str {
        match self {
            Self::Receive => "receive_locations",
            Self::Send => "send_ports",
        }
    }
}

/// One Receive Location or Send Port, and the node that takes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    pub placed: Placed,
    /// The Application that designs it.
    pub application: String,
    pub name: String,
    pub node: String,
}

impl Placement {
    /// Its name as a folder's, or a far end user's: lower case, every
    /// character but a letter or a digit a dash.
    #[must_use]
    pub fn folded(&self) -> String {
        self.name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect()
    }
}

/// How one placement is bound: the address, whose scheme chooses the
/// technology, the settings beyond it, and whether it accepts what the
/// circumstance of its drop identifies — a Receive Location's, where
/// nothing else is presented.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub address: String,
    pub settings: Vec<(String, String)>,
    pub accepting: bool,
}

/// The nodes of `present` with the roles `cluster` declares for each, in
/// the file's order: what an environment's nodes are placed by.
///
/// # Errors
///
/// Where the file does not read, or `present` names a node it does not
/// declare.
pub fn present(cluster: &str, present: &[String]) -> Result<Vec<(String, Vec<String>)>, String> {
    let declared = cluster::roles(cluster)?;
    if let Some(stranger) = present
        .iter()
        .find(|name| !declared.iter().any(|(node, _)| node == *name))
    {
        let names: Vec<&str> = declared.iter().map(|(node, _)| node.as_str()).collect();
        return Err(format!(
            "{stranger} is no node of the cluster's file, which declares {}",
            names.join(", ")
        ));
    }
    Ok(declared
        .into_iter()
        .filter(|(node, _)| present.contains(node))
        .collect())
}

/// Every Receive Location and Send Port `cluster` designs, each on the
/// node of `nodes` — a node and its roles — the module's round robin gives
/// it; one no node takes is left unplaced.
///
/// # Errors
///
/// Where the file's Applications do not read.
pub fn placements(
    cluster: &str,
    nodes: &[(String, Vec<String>)],
) -> Result<Vec<Placement>, String> {
    let holding = |any: &[&str]| -> Vec<String> {
        nodes
            .iter()
            .filter(|(_, roles)| roles.iter().any(|role| any.contains(&role.as_str())))
            .map(|(node, _)| node.clone())
            .collect()
    };
    let lists = [
        (Placed::Receive, holding(&["receiving", "executing"])),
        (Placed::Send, holding(&["sending", "executing"])),
    ];
    let mut placed = Vec::new();
    let mut next = [0, 0];
    for application in section::applications(cluster)? {
        let names = [
            application
                .receive_locations
                .iter()
                .map(|one| &one.name)
                .collect::<Vec<_>>(),
            application.send_ports.iter().map(|one| &one.name).collect(),
        ];
        for (at, ((kind, nodes), names)) in lists.iter().zip(names).enumerate() {
            for name in names {
                if let Some(node) = nodes.get(next[at] % nodes.len().max(1)) {
                    placed.push(Placement {
                        placed: *kind,
                        application: application.name.clone(),
                        name: name.clone(),
                        node: node.clone(),
                    });
                }
                next[at] += 1;
            }
        }
    }
    Ok(placed)
}

/// The bindings of `placements`, each bound as `bound` says, as TOML to
/// stand beside the cluster's file: one `[[applications]]` for each
/// Application placed.
#[must_use]
pub fn written(placements: &[Placement], bound: impl Fn(&Placement) -> Binding) -> String {
    let mut text = String::new();
    let mut application: Option<&str> = None;
    for placement in placements {
        if application != Some(placement.application.as_str()) {
            application = Some(&placement.application);
            text.push_str(&format!(
                "\n[[applications]]\nname = {}\n",
                quoted(&placement.application)
            ));
        }
        let binding = bound(placement);
        let list = placement.placed.list();
        text.push_str(&format!(
            "\n[[applications.{list}]]\nname = {}\nnode = {}\nstart = true\n\
             address = {}\n",
            quoted(&placement.name),
            quoted(&placement.node),
            quoted(&binding.address),
        ));
        if !binding.settings.is_empty() {
            text.push_str(&format!("[applications.{list}.settings]\n"));
            for (key, value) in &binding.settings {
                text.push_str(&format!("{key} = {}\n", quoted(value)));
            }
        }
        if binding.accepting {
            text.push_str(&format!(
                "[applications.{list}.accept]\nmechanism = [\"circumstance\"]\n"
            ));
        }
    }
    text
}

/// Each placement over SFTP on the Parties' far end at `host` — `host` or
/// `host:port`, [`FAR_END_PORT`] where it names none — as a user of its
/// own, the user's directory its folder there; a Receive Location accepting
/// the circumstance of what it fetches, as a drop folder's.
pub fn far_end(host: &str) -> impl Fn(&Placement) -> Binding + '_ {
    move |placement| Binding {
        address: format!("sftp://{}@{}", placement.folded(), with_port(host)),
        settings: vec![("timeout".to_string(), "10s".to_string())],
        accepting: placement.placed == Placed::Receive,
    }
}

/// `host` with [`FAR_END_PORT`] where it names no port of its own.
fn with_port(host: &str) -> String {
    let named = host
        .rsplit_once(':')
        .is_some_and(|(before, port)| !before.ends_with(':') && port.parse::<u16>().is_ok());
    if named {
        host.to_string()
    } else if host.contains(':') {
        format!("[{host}]:{FAR_END_PORT}")
    } else {
        format!("{host}:{FAR_END_PORT}")
    }
}

/// `cluster` with every Application it designs placed over `present` on
/// the Parties' far end at `host`: the file `xmip-service --slice --bind`
/// slices.
///
/// # Errors
///
/// As [`placements`].
pub fn at_far_end(cluster: &str, present: &[String], host: &str) -> Result<String, String> {
    let placed = placements(cluster, &self::present(cluster, present)?)?;
    Ok(format!("{cluster}\n{}", written(&placed, far_end(host))))
}

/// `text` as a TOML string, quoted and escaped as TOML writes it.
#[must_use]
pub fn quoted(text: &str) -> String {
    toml::Value::String(text.to_string()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::test_cluster;

    /// A file of four nodes and one Application of three Receive Locations
    /// and two Send Ports.
    fn file() -> (String, [String; 4]) {
        let cluster = test_cluster();
        let names = [0, 1, 2, 3].map(|place| cluster.node(place).name.clone());
        let [first, second, third, fourth] = &names;
        let text = format!(
            "[nodes.{first}]\nroles = \"receiving\"\n[nodes.{second}]\nroles = \"executing\"\n\
             [nodes.{third}]\nroles = \"sending\"\n[nodes.{fourth}]\nroles = \"storage\"\n\
             [[xmip_applications]]\nname = \"Orders\"\n\
             [[xmip_applications.receive_ports]]\nname = \"In\"\n\
             [[xmip_applications.receive_locations]]\nname = \"From A\"\nreceive_port = \"In\"\n\
             depth = \"light\"\n\
             [[xmip_applications.receive_locations]]\nname = \"From B\"\nreceive_port = \"In\"\n\
             depth = \"light\"\n\
             [[xmip_applications.receive_locations]]\nname = \"From C\"\nreceive_port = \"In\"\n\
             depth = \"light\"\n\
             [[xmip_applications.send_ports]]\nname = \"To A\"\n\
             [[xmip_applications.send_ports]]\nname = \"To B\"\n"
        );
        (text, names)
    }

    #[test]
    fn locations_go_round_robin_over_the_nodes_present_that_take_them() {
        let (text, [first, second, third, fourth]) = file();
        let all = [&first, &second, &third, &fourth].map(|name| name.clone());
        let placed = placements(&text, &present(&text, &all).expect("declared")).expect("placed");
        let on: Vec<(&str, &str)> = placed
            .iter()
            .map(|one| (one.name.as_str(), one.node.as_str()))
            .collect();
        assert_eq!(
            on,
            [
                ("From A", first.as_str()),
                ("From B", second.as_str()),
                ("From C", first.as_str()),
                ("To A", second.as_str()),
                ("To B", third.as_str()),
            ]
        );
        // Only the nodes present take anything.
        let two = present(&text, &[fourth.clone(), second.clone()]).expect("declared");
        let alone = placements(&text, &two).expect("placed");
        assert!(alone.iter().all(|one| one.node == second), "{alone:?}");
        assert_eq!(alone.len(), 5);
        let refused = present(&text, &["no-such-node".to_string()]).expect_err("stranger");
        assert!(refused.contains("no-such-node is no node"), "{refused}");
    }

    #[test]
    fn the_far_end_binds_each_as_a_user_of_its_own_and_every_node_slices() {
        let (text, names) = file();
        let whole = at_far_end(&text, &names, "10.77.0.1").expect("placed");
        let parsed: toml::Table = whole.parse().expect("TOML");
        let bound = &parsed["applications"][0];
        let first = &bound["receive_locations"][0];
        assert_eq!(
            first["address"].as_str(),
            Some("sftp://from-a@10.77.0.1:7422")
        );
        assert!(first.get("transport").is_none(), "the scheme chooses it");
        assert!(
            first["settings"].get("user").is_none(),
            "the address names the user"
        );
        assert_eq!(
            first["accept"]["mechanism"][0].as_str(),
            Some("circumstance")
        );
        let port = &bound["send_ports"][0];
        assert_eq!(port["address"].as_str(), Some("sftp://to-a@10.77.0.1:7422"));
        assert!(port.get("accept").is_none(), "a Send Port accepts nothing");
        for node in &names {
            crate::slice_named(&whole, node, "placed").expect("slices");
        }
        assert_eq!(with_port("ca:9000"), "ca:9000");
        assert_eq!(with_port("::1"), "[::1]:7422");
    }
}
