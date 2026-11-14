//! Where a cluster's Xmip Applications run: each Receive Location, Send
//! Port and Work Process the cluster's file designs, placed on the nodes an
//! environment has by what each declares — the one placement for every
//! environment that writes no bindings by hand (ADR-0028, amendments
//! 2026-10-21 and 2026-10-22; ADR-0056, amendment 2026-11-13). The work
//! requires its stage served ([`Placed::requirement`]): a Receive Location
//! the receive stage, a Send Port the send stage, a Work Process the
//! process stage, and a node satisfies that where one of its roles serves
//! it, executing serving every stage (`node::Capability::satisfies`). Each
//! Receive Location and Send Port goes to one node, round robin in the
//! file's order over the nodes present that satisfy it (`xcluster::place`,
//! one rotation for each of the two); each Work Process to every node
//! present that satisfies it (`xcluster::every`), which share its turns
//! through Xmip Storage. Work no node present satisfies is refused, naming
//! the requirement, every node considered and the work (ADR-0056 clause
//! 4), never left unplaced. Nothing reads a node's name.
//!
//! The Essentials places the nodes it starts on one computer by it, over
//! folders of its run; `xmip-service --slice --bind` places a node on a
//! machine of its own by it, over the Parties' far end the Essentials
//! serves ([`far_end`]); and the Essentials drives what it placed, so the
//! two can never place differently.

use node::{Requirement, Stage};
use xcluster::{Candidate, Rotation, Unplaced};

use crate::{cluster, section};

/// The port the Parties' far end is served on where an address names none.
pub const FAR_END_PORT: u16 = 7422;

/// The three lists of an Application a placement is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placed {
    /// A Receive Location.
    Receive,
    /// A Send Port.
    Send,
    /// A Work Process.
    Process,
}

impl Placed {
    /// The list's key in a binding.
    #[must_use]
    pub const fn list(self) -> &'static str {
        match self {
            Self::Receive => "receive_locations",
            Self::Send => "send_ports",
            Self::Process => "work_processes",
        }
    }

    /// The stage of the message path the work serves.
    #[must_use]
    pub const fn stage(self) -> Stage {
        match self {
            Self::Receive => Stage::Receive,
            Self::Send => Stage::Send,
            Self::Process => Stage::Process,
        }
    }

    /// What the work requires of a node that takes it (ADR-0056 clause 3):
    /// its stage served. No design says its work reaches off the machine,
    /// so none requires the online capability yet.
    #[must_use]
    pub const fn requirement(self) -> Requirement {
        Requirement::serving(self.stage())
    }

    /// What one is called in a refusal.
    const fn said(self) -> &'static str {
        match self {
            Self::Receive => "Receive Location",
            Self::Send => "Send Port",
            Self::Process => "Work Process",
        }
    }
}

/// One Receive Location, Send Port or Work Process, and a node that takes
/// it: a Work Process is placed once for each node that runs it.
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
/// technology, the settings beyond it, the secret it presents where it
/// names one, and whether it accepts what the circumstance of its drop
/// identifies — a Receive Location's, where nothing else is presented.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub address: String,
    pub settings: Vec<(String, String)>,
    /// The name of the secret it presents (`secret = "<name>"`).
    pub secret: Option<String>,
    pub accepting: bool,
}

/// The nodes of `present` with what `cluster` declares for each
/// (`cluster::capabilities`), in the file's order: what an environment's
/// nodes are placed by.
///
/// # Errors
///
/// Where the file does not read, or `present` names a node it does not
/// declare.
pub fn present(
    cluster: &str,
    present: &[String],
) -> Result<Vec<(String, node::Capability)>, String> {
    let declared = cluster::capabilities(cluster)?;
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

/// Every Receive Location, Send Port and Work Process `cluster` designs,
/// each Application's in that order and in the file's, placed over `nodes`
/// by what each declares: a Receive Location or Send Port on the one node
/// the rotation of its kind gives it, a Work Process on every node
/// satisfying it.
///
/// # Errors
///
/// Where the file's Applications do not read, or work no node of `nodes`
/// satisfies: REFUSED, a sentence for each requirement unmet, naming it,
/// every node considered with what it declares, and the work not placed.
pub fn placements<C: Candidate>(cluster: &str, nodes: &[C]) -> Result<Vec<Placement>, String> {
    let (receive, send) = (Rotation::new(), Rotation::new());
    let mut placed = Vec::new();
    let mut refused: Vec<(Unplaced, Vec<String>)> = Vec::new();
    for application in section::applications(cluster)? {
        let receives = application
            .receive_locations
            .iter()
            .map(|one| (Placed::Receive, &one.name));
        let sends = application
            .send_ports
            .iter()
            .map(|one| (Placed::Send, &one.name));
        let processes = application
            .work_processes
            .iter()
            .map(|one| (Placed::Process, &one.name));
        for (kind, name) in receives.chain(sends).chain(processes) {
            let requirement = kind.requirement();
            let on = match kind {
                Placed::Receive => {
                    xcluster::place(requirement, nodes, &receive).map(|one| vec![one])
                }
                Placed::Send => xcluster::place(requirement, nodes, &send).map(|one| vec![one]),
                Placed::Process => xcluster::every(requirement, nodes),
            };
            match on {
                Ok(on) => placed.extend(on.into_iter().map(|node| Placement {
                    placed: kind,
                    application: application.name.clone(),
                    name: name.clone(),
                    node: node.node().to_string(),
                })),
                Err(unplaced) => {
                    let work = format!("the {} '{name}' of '{}'", kind.said(), application.name);
                    match refused.iter_mut().find(|(held, _)| *held == unplaced) {
                        Some((_, works)) => works.push(work),
                        None => refused.push((unplaced, vec![work])),
                    }
                }
            }
        }
    }
    if refused.is_empty() {
        return Ok(placed);
    }
    let said: Vec<String> = refused
        .iter()
        .map(|(unplaced, works)| format!("{unplaced} Not placed: {}.", works.join(", ")))
        .collect();
    Err(said.join("\n"))
}

/// The bindings of `placements`, each bound as `bound` says, as TOML to
/// stand beside the cluster's file: one `[[applications]]` for each
/// Application placed. A Work Process is bound by its name and its node
/// alone, and `bound` is not asked of it.
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
        let list = placement.placed.list();
        if placement.placed == Placed::Process {
            text.push_str(&format!(
                "\n[[applications.{list}]]\nname = {}\nnode = {}\n",
                quoted(&placement.name),
                quoted(&placement.node),
            ));
            continue;
        }
        let binding = bound(placement);
        text.push_str(&format!(
            "\n[[applications.{list}]]\nname = {}\nnode = {}\nstart = true\n\
             address = {}\n",
            quoted(&placement.name),
            quoted(&placement.node),
            quoted(&binding.address),
        ));
        if let Some(secret) = &binding.secret {
            text.push_str(&format!("secret = {}\n", quoted(secret)));
        }
        if !binding.settings.is_empty() {
            text.push_str(&format!("[applications.{list}.settings]\n"));
            for (key, value) in &binding.settings {
                text.push_str(&format!("{key} = {}\n", quoted(value)));
            }
        }
        if binding.accepting {
            text.push_str(&format!(
                "[applications.{list}.accept]\nmechanism = [\"circumstance\"]\ntransport = \"claimed\"\n"
            ));
        }
    }
    text
}

/// Each placement over SFTP on the Parties' far end at `host` — `host` or
/// `host:port`, [`FAR_END_PORT`] where it names none — as a user of its
/// own, the user's directory its folder there, presenting the password its
/// secret [`far_end_secret`] holds, which the far end checks; a Receive
/// Location accepting the circumstance of what it fetches, as a drop
/// folder's.
pub fn far_end(host: &str) -> impl Fn(&Placement) -> Binding + '_ {
    move |placement| Binding {
        address: format!("sftp://{}@{}", placement.folded(), with_port(host)),
        settings: vec![("timeout".to_string(), "10s".to_string())],
        secret: Some(far_end_secret(placement)),
        accepting: placement.placed == Placed::Receive,
    }
}

/// The name of the secret a placement presents to the Parties' far end:
/// `far-end.<folded name>`, set on its node by whoever serves the far end.
#[must_use]
pub fn far_end_secret(placement: &Placement) -> String {
    format!("far-end.{}", placement.folded())
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

    /// The roles of the five nodes [`file`] declares, in the test cluster's
    /// order of its first five nodes.
    const ROLES: [&str; 5] = ["receiving", "executing", "sending", "storage", "processing"];

    /// A file of five nodes, each declaring the role `roles` gives it, and
    /// one Application of three Receive Locations, two Send Ports and a
    /// Work Process.
    fn file(roles: [&str; 5]) -> (String, [String; 5]) {
        let cluster = test_cluster();
        let names = [0, 1, 2, 3, 4].map(|place| cluster.node(place).name.clone());
        let nodes: String = names
            .iter()
            .zip(roles)
            .map(|(name, role)| format!("[nodes.{name}]\nroles = \"{role}\"\n"))
            .collect();
        let text = format!(
            "{nodes}[[xmip_applications]]\nname = \"Orders\"\n\
             [[xmip_applications.receive_ports]]\nname = \"In\"\n\
             [[xmip_applications.receive_locations]]\nname = \"From A\"\nreceive_port = \"In\"\n\
             depth = \"light\"\n\
             [[xmip_applications.receive_locations]]\nname = \"From B\"\nreceive_port = \"In\"\n\
             depth = \"light\"\n\
             [[xmip_applications.receive_locations]]\nname = \"From C\"\nreceive_port = \"In\"\n\
             depth = \"light\"\n\
             [[xmip_applications.send_ports]]\nname = \"To A\"\n\
             [[xmip_applications.send_ports]]\nname = \"To B\"\n\
             [[xmip_applications.work_processes]]\nname = \"Confirm\"\nmodule = \"confirming\"\n\
             unit = \"confirm\"\n[xmip_applications.work_processes.settings]\nlimit = \"3\"\n"
        );
        (text, names)
    }

    /// What `placed` puts where: each piece of work by its name, on its
    /// node.
    fn on(placed: &[Placement]) -> Vec<(&str, &str)> {
        placed
            .iter()
            .map(|one| (one.name.as_str(), one.node.as_str()))
            .collect()
    }

    #[test]
    fn work_goes_round_the_nodes_serving_its_stage_and_a_work_process_to_each() {
        let (text, names) = file(ROLES);
        let [first, second, third, fourth, fifth] = names.each_ref().map(String::as_str);
        let placed = placements(&text, &present(&text, &names).expect("declared")).expect("placed");
        assert_eq!(
            on(&placed),
            [
                ("From A", first),
                ("From B", second),
                ("From C", first),
                ("To A", second),
                ("To B", third),
                ("Confirm", second),
                ("Confirm", fifth),
            ]
        );
        assert!(placed[5..].iter().all(|one| one.placed == Placed::Process));
        // Only the nodes present take anything.
        let two = present(&text, &[fourth.to_string(), second.to_string()]).expect("declared");
        let alone = placements(&text, &two).expect("placed");
        assert!(alone.iter().all(|one| one.node == second), "{alone:?}");
        assert_eq!(alone.len(), 6);
        let refused = present(&text, &["no-such-node".to_string()]).expect_err("stranger");
        assert!(refused.contains("no-such-node is no node"), "{refused}");
    }

    #[test]
    fn a_node_is_chosen_by_its_roles_and_never_its_name() {
        let (text, names) = file(["sending", "processing", "receiving", "storage", "executing"]);
        let [first, second, third, _, fifth] = names.each_ref().map(String::as_str);
        let placed = placements(&text, &present(&text, &names).expect("declared")).expect("placed");
        assert_eq!(
            on(&placed),
            [
                ("From A", third),
                ("From B", fifth),
                ("From C", third),
                ("To A", first),
                ("To B", fifth),
                ("Confirm", second),
                ("Confirm", fifth),
            ]
        );
    }

    #[test]
    fn work_no_node_present_satisfies_is_refused_naming_both_sides_and_the_work() {
        let (text, names) = file(ROLES);
        let (first, fourth) = (&names[0], &names[3]);
        let two = [first.clone(), fourth.clone()];
        let refused = placements(&text, &present(&text, &two).expect("declared"))
            .expect_err("nothing sends or processes");
        let considered =
            format!("considered {first} (receiving; offline), {fourth} (storage; offline).");
        assert_eq!(
            refused,
            format!(
                "REFUSED: no node satisfies work requiring serving Send; {considered} Not placed: \
                 the Send Port 'To A' of 'Orders', the Send Port 'To B' of 'Orders'.\n\
                 REFUSED: no node satisfies work requiring serving Work Process; {considered} \
                 Not placed: the Work Process 'Confirm' of 'Orders'."
            )
        );
        assert_eq!(
            at_far_end(&text, &two, "10.77.0.1").expect_err("refused the same"),
            refused,
            "--slice --bind refuses as the placement does"
        );
        let none: [(String, node::Capability); 0] = [];
        let alone = placements(&text, &none).expect_err("no node at all");
        assert!(alone.contains("considered none. Not placed: the Receive Location 'From A'"));
    }

    #[test]
    fn the_far_end_binds_each_as_a_user_of_its_own_and_every_node_slices() {
        let (text, names) = file(ROLES);
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
        assert_eq!(first["secret"].as_str(), Some("far-end.from-a"));
        assert_eq!(
            first["accept"]["mechanism"][0].as_str(),
            Some("circumstance")
        );
        let port = &bound["send_ports"][0];
        assert_eq!(port["address"].as_str(), Some("sftp://to-a@10.77.0.1:7422"));
        assert!(port.get("accept").is_none(), "a Send Port accepts nothing");
        let process = bound["work_processes"]
            .as_array()
            .expect("its Work Processes");
        let nodes: Vec<&str> = process
            .iter()
            .filter_map(|one| one["node"].as_str())
            .collect();
        assert_eq!(nodes, [names[1].as_str(), names[4].as_str()]);
        assert!(
            process
                .iter()
                .all(|one| one.as_table().is_some_and(|it| it.len() == 2)),
            "a Work Process is bound by its name and node alone: {process:?}"
        );
        for node in &names {
            crate::slice_named(&whole, node, "placed").expect("slices");
        }
        assert_eq!(with_port("ca:9000"), "ca:9000");
        assert_eq!(with_port("::1"), "[::1]:7422");
    }

    /// `xmip-service --slice --bind` writes what the placement places,
    /// which the Essentials drives: the two place identically.
    #[test]
    fn the_far_end_binds_exactly_what_the_placement_places() {
        let (text, names) = file(ROLES);
        let placed = placements(&text, &present(&text, &names).expect("declared")).expect("placed");
        let whole = at_far_end(&text, &names, "10.77.0.1").expect("placed");
        let parsed: toml::Table = whole.parse().expect("TOML");
        let bound = &parsed["applications"][0];
        let mut written = Vec::new();
        for kind in [Placed::Receive, Placed::Send, Placed::Process] {
            for one in bound[kind.list()].as_array().expect("a list") {
                let said = |key: &str| one[key].as_str().expect("said").to_string();
                written.push((kind, said("name"), said("node")));
            }
        }
        let expected: Vec<(Placed, String, String)> = placed
            .into_iter()
            .map(|one| (one.placed, one.name, one.node))
            .collect();
        assert_eq!(written, expected);
    }
}
