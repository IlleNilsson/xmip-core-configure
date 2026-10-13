//! An Xmip Application held as a section of the cluster's `xmip.toml`
//! (ADR-0064 and ADR-0031, amendments 2026-10-03: *Developers, Operators
//! deal with the one xmip.toml file*, its Xmip Applications among its
//! sections). There is no other place an Application is written.
//!
//! ```toml
//! [[xmip_applications]]
//! name = "Orders"
//!
//! [[xmip_applications.send_ports]]
//! name = "Billing"
//!
//! [[xmip_applications.subscriptions]]
//! id = "billing"
//! destination = { send-port = "Billing" }
//! filter = "MessageType = 'Order'"
//!
//! [[applications]]          # the binding: the section of its name
//! name = "Orders"
//! ```
//!
//! A section is read by the one reading of an Application,
//! [`crate::parse_application`] ([`ApplicationSection::application`]), so
//! nothing restates its shape. A binding binds the section of its name
//! ([`crate::binding`]), and the slice gives a node the sections its
//! bindings name ([`crate::cluster::slice`]).

use serde::{Deserialize, Serialize};
use toml::{Table, Value};

use crate::application::XmipApplication;

/// The key the sections are listed under, in a cluster's file and in the
/// slice a node reads.
pub const SECTIONS: &str = "xmip_applications";

/// One `[[xmip_applications]]` entry, as written.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ApplicationSection(pub Table);

// A table can hold a float, and a float is not Eq; an Application holds
// none (its reading refuses every key it does not define), so equality is
// the tables' own, as for `LocationSettings`.
impl Eq for ApplicationSection {}

impl ApplicationSection {
    /// The Application's name, the entry's `name`.
    #[must_use]
    pub fn name(&self) -> &str {
        self.0
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
    }

    /// The Application the section is, read by the one reading of an
    /// Application.
    ///
    /// # Errors
    /// The reader's words, opened with the section's name, when the section
    /// does not read as an Application.
    pub fn application(&self) -> Result<XmipApplication, String> {
        let text = self.text()?;
        crate::parse_application(&text)
            .map_err(|error| format!("[[{SECTIONS}]] '{}': {error}", self.name()))
    }

    /// The section's own text, its keys at the top: what
    /// [`crate::parse_application`] reads.
    ///
    /// # Errors
    /// When the section cannot be written as TOML.
    pub fn text(&self) -> Result<String, String> {
        toml::to_string(&self.0).map_err(|error| error.to_string())
    }
}

/// What `[[xmip_applications]]` alone is read as.
#[derive(Deserialize)]
struct Held {
    #[serde(default)]
    xmip_applications: Vec<ApplicationSection>,
}

/// Every Xmip Application `source` holds as a section, in the order
/// written, each read by the one reading of an Application: a cluster's
/// file or a node's slice, read for `[[xmip_applications]]` alone.
///
/// # Errors
/// When the text is not TOML or a section does not read as an Application.
pub fn applications(source: &str) -> Result<Vec<XmipApplication>, String> {
    let held: Held = toml::from_str(source).map_err(|error| error.to_string())?;
    held.xmip_applications
        .iter()
        .map(ApplicationSection::application)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::tests::ORDERS;

    fn section(text: &str) -> ApplicationSection {
        ApplicationSection(text.parse().expect("TOML"))
    }

    #[test]
    fn a_section_reads_as_the_application_it_is() {
        let orders = section(ORDERS);
        let application = orders.application().expect("reads");

        assert_eq!(orders.name(), "Orders");
        assert_eq!(application.name, "Orders");
        assert_eq!(application.subscriptions[0].id, "billing");
        assert_eq!(
            crate::parse_application(&orders.text().expect("writes")).expect("reads"),
            application
        );
    }

    #[test]
    fn a_section_holding_what_an_application_does_not_define_is_refused_by_name() {
        let refused = section(&format!("{ORDERS}\n[[send_port]]\nname = \"x\"\n"))
            .application()
            .expect_err("refused");

        assert!(
            refused.starts_with("[[xmip_applications]] 'Orders'"),
            "{refused}"
        );
        assert!(refused.contains("send_port"), "{refused}");
    }

    #[test]
    fn every_section_a_file_holds_reads_in_order() {
        let mut held = Table::new();
        let orders = Value::Table(section(ORDERS).0);
        held.insert(SECTIONS.into(), Value::Array(vec![orders.clone(), orders]));
        let text = toml::to_string(&held).expect("writes");
        let read = applications(&text).expect("reads");
        assert_eq!(read.len(), 2);
        assert_eq!(read[0], section(ORDERS).application().expect("reads"));
        assert!(
            applications("[service]\nname = \"n\"\n")
                .expect("none")
                .is_empty()
        );
    }
}
