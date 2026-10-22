//! Prepare, Promote, Demote and Transformation as calls to compiled units,
//! and where each is attached (ADR-0066, amendment 2026-10-22).
//!
//! The owner, 2026-10-22: *They are tricky but not impossible. They are
//! compiled units. They just have to be referred to in the VS Code
//! extension, ending up in the TOML configuration.* A unit is a function a
//! Module provides (`abi::ModuleUnit`), and a step entry is a call to it
//! with arguments: the `[[modules]]` entry holding it, the unit's name as
//! that module's manifest declares it, and the `settings` the call gives,
//! held to what the unit declares.
//!
//! ```toml
//! [[transformations]]
//! name = "EdifactOrderToOrder"
//! module = "orders"
//! unit = "EdifactToOrder"
//! settings = { version = "D96A" }
//!
//! [[receive_locations]]
//! name = "OrdersEdi"
//! prepare = ["Decompress"]           # Preparation Steps, in order
//! transform = "EdifactOrderToOrder"
//! promote = "OrderNumbers"           # receive alone; demote, send alone
//! ```
//!
//! A node's document and an Xmip Application section each hold the four
//! lists — `[[preparation_steps]]`, `[[promotions]]`, `[[demotions]]` and
//! `[[transformations]]` — and a Receive Port, Receive Location, Send Port
//! or Send Location attaches entries of its own document's lists by name:
//! on receive the Location's run, then the Port's; on send the Port's, then
//! the Location's (`runtime-model.md` section 20). A unit lists the stages
//! it serves, `receive`, `process` and `send`, and is handed the one it is
//! called at: it is attached on receive only where it serves `receive`, on
//! send only where it serves `send`, and called from an Xmip Work Process,
//! whose `module` and `unit` name its flow, only where it serves `process`.
//! A prepare unit at each is a Receive, Process or Send Preparation; a
//! Transformation is never a Preparation (the owner, 2026-10-22).
//!
//! What is refused, in words, at validation and as a node starts
//! ([`problems`]): a module no `[[modules]]` entry declares, a unit its
//! manifest does not declare or declares as another capability, a call's
//! settings its unit's declaration refuses, an attachment naming no entry,
//! a unit attached where its stage does not fit, and a Promotion on send or
//! a Demotion on receive. Running a unit is decided, not built: a node
//! refuses to start what attaches one (`xmip-core-runtime`).

mod problems;

use abi::{ModuleUnit, UnitCapability};
use serde::{Deserialize, Serialize};

pub use problems::problems;

use crate::modules::ModuleConfiguration;
use crate::{ConfiguredLocation, LocationSettings, XmipApplication, XmipConfigurationDocument};

/// One call to a compiled unit: a Preparation Step, a Promotion, a
/// Demotion or a Transformation, by the list it is in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepEntry {
    /// What an attachment names it by.
    pub name: String,
    /// The `[[modules]]` entry holding its unit.
    pub module: String,
    /// The unit, as that module's manifest names it.
    pub unit: String,
    /// What the call gives, held to the unit's declaration.
    #[serde(default, skip_serializing_if = "LocationSettings::is_empty")]
    pub settings: LocationSettings,
}

/// The four lists, each with the capability its units must have and how a
/// person names one of it.
pub const LISTS: [(&str, UnitCapability, &str); 4] = [
    (
        "preparation_steps",
        UnitCapability::Prepare,
        "Preparation Step",
    ),
    ("promotions", UnitCapability::Promote, "Promotion"),
    ("demotions", UnitCapability::Demote, "Demotion"),
    (
        "transformations",
        UnitCapability::Transform,
        "Transformation",
    ),
];

/// What an artifact attaches, by name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Attached<'a> {
    pub prepare: &'a [String],
    pub transform: Option<&'a str>,
    pub promote: Option<&'a str>,
    pub demote: Option<&'a str>,
}

impl Attached<'_> {
    /// Whether it attaches anything.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.prepare.is_empty()
            && self.transform.is_none()
            && self.promote.is_none()
            && self.demote.is_none()
    }

    /// Each attachment: the list it names an entry of, and the name.
    #[must_use]
    pub fn named(&self) -> Vec<(&'static str, &str)> {
        let mut named: Vec<(&'static str, &str)> = self
            .prepare
            .iter()
            .map(|name| (LISTS[0].0, name.as_str()))
            .collect();
        for (list, name) in [
            (LISTS[1].0, self.promote),
            (LISTS[2].0, self.demote),
            (LISTS[3].0, self.transform),
        ] {
            named.extend(name.map(|name| (list, name)));
        }
        named
    }
}

impl ConfiguredLocation {
    /// What the Location attaches.
    #[must_use]
    pub fn attached(&self) -> Attached<'_> {
        Attached {
            prepare: &self.prepare,
            transform: self.transform.as_deref(),
            promote: self.promote.as_deref(),
            demote: self.demote.as_deref(),
        }
    }
}

/// A document's four lists.
#[derive(Clone, Copy)]
pub struct Steps<'a> {
    pub lists: [&'a [StepEntry]; 4],
}

impl<'a> Steps<'a> {
    /// A node's.
    #[must_use]
    pub fn of_node(document: &'a XmipConfigurationDocument) -> Self {
        Self {
            lists: [
                &document.preparation_steps,
                &document.promotions,
                &document.demotions,
                &document.transformations,
            ],
        }
    }

    /// An Xmip Application's.
    #[must_use]
    pub fn of_application(application: &'a XmipApplication) -> Self {
        Self {
            lists: [
                &application.preparation_steps,
                &application.promotions,
                &application.demotions,
                &application.transformations,
            ],
        }
    }

    /// The entry of `list` called `name`.
    #[must_use]
    pub fn entry(&self, list: &str, name: &str) -> Option<&'a StepEntry> {
        let at = LISTS.iter().position(|(key, _, _)| *key == list)?;
        self.lists[at].iter().find(|entry| entry.name == name)
    }
}

/// The unit `entry` calls, where the module holding it is declared in
/// `modules` and declares it.
#[must_use]
pub fn unit_of<'m>(
    entry: &StepEntry,
    modules: &'m [ModuleConfiguration],
) -> Option<&'m ModuleUnit> {
    modules
        .iter()
        .find(|module| module.name == entry.module)
        .and_then(|module| module.unit(&entry.unit))
}

/// What a node starting by `document` and the `applications` it binds would
/// have to run of a compiled unit, one sentence each: every Port and
/// Location that attaches a call. Running a unit is decided, not built, so
/// a node refuses to start any (`xmip-core-runtime`, startup phase 3).
#[must_use]
pub fn unrun(
    document: &XmipConfigurationDocument,
    applications: &[XmipApplication],
) -> Vec<String> {
    let bound = document.applications.iter().flat_map(|binding| {
        let receives = binding
            .receive_locations
            .iter()
            .map(|bound| &bound.location);
        receives.chain(binding.send_ports.iter().map(|bound| &bound.location))
    });
    let own = document
        .receive_locations
        .iter()
        .chain(&document.send_locations);
    let mut attaching: Vec<String> = own
        .chain(bound)
        .filter(|location| !location.attached().is_empty())
        .map(|location| format!("the Location '{}'", location.name))
        .collect();
    for application in applications {
        let of = |kind: &str, name: &str| format!("the {kind} '{name}' of '{}'", application.name);
        let ports = application.receive_ports.iter();
        attaching.extend(
            ports
                .filter(|port| !port.attached().is_empty())
                .map(|port| of("Receive Port", &port.name)),
        );
        let receives = application.receive_locations.iter();
        attaching.extend(
            receives
                .filter(|location| !location.attached().is_empty())
                .map(|location| of("Receive Location", &location.name)),
        );
        let sends = application.send_ports.iter();
        attaching.extend(
            sends
                .filter(|port| !port.attached().is_empty())
                .map(|port| of("Send Port", &port.name)),
        );
    }
    attaching
        .into_iter()
        .map(|what| {
            format!(
                "{what} attaches a Preparation Step, Promotion, Demotion or Transformation, and \
                 this runtime runs no compiled unit yet"
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::unrun;
    use crate::parse_toml;

    #[test]
    fn a_node_attaching_a_call_is_refused_until_units_run() {
        let head = "[service]\nname = \"n\"\ncluster_name = \"c\"\nnode_name = \"d\"\n\n\
                    [[receive_locations]]\nname = \"drop\"\ntransport = \"t\"\naddress = \"a\"\n";
        let plain = parse_toml(head).expect("reads");
        assert_eq!(unrun(&plain, &[]), [] as [String; 0]);

        let attaching = parse_toml(&format!("{head}transform = \"Enriched\"\n")).expect("reads");
        let refused = unrun(&attaching, &[]);
        assert!(refused[0].contains("the Location 'drop'"), "{refused:?}");
        assert!(
            refused[0].contains("runs no compiled unit yet"),
            "{refused:?}"
        );
    }
}
