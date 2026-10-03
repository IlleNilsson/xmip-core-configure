//! The Ports of an Xmip Application and the Locations it designs at them,
//! with the runtime's settings the configuration gives each (ADR-0031,
//! amendment 2026-10-01; `runtime-model.md` section 20, *Where the
//! runtime's settings are configured*).
//!
//! ```toml
//! [[xmip_applications.receive_ports]]
//! name = "Invoices"
//!
//! [[xmip_applications.receive_locations]]
//! name         = "InvoicesHttp"
//! receive_port = "Invoices"         # a Location without a Port is refused
//! interaction  = "data-transfer"    # composite | data-transfer | batch-load
//! depth        = "light"            # transfer | light | context
//!
//! [[xmip_applications.send_ports]]
//! name            = "ErpOut"
//! send_locations  = ["ErpPrimary", "ErpBackup"]       # tried in order
//! retry           = { attempts = 3, backoff = "5s" }  # on the active Location
//! failover        = "next"          # next | none
//! execution_style = "sequential"
//! order_key       = "party"         # for example, per Party
//! on_failure      = "block"         # block | skip
//! ```
//!
//! A Receive Location names its Receive Port and states its interaction and
//! its processing depth (`runtime-model.md` sections 6 and 7); one that does
//! not, or names a Port the Application does not declare, is a problem of
//! the Application, refused at startup phase 3 with the rest of it. A Send
//! Port states its policy (section 10), and a Sequential one without
//! `on_failure` is refused (section 3: no silent default). A word a key does
//! not have is refused as the Application is read.

use serde::{Deserialize, Serialize};

use crate::ExecutionStyle;
use crate::application::DesignedElement;

/// What the caller expects back (`runtime-model.md` section 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Interaction {
    /// A response composed through configured Xmip work.
    Composite,
    /// Acknowledged once acceptance succeeds.
    DataTransfer,
    /// Accepted and acknowledged for later processing.
    BatchLoad,
}

/// How far Xmip interprets the content (`runtime-model.md` section 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Depth {
    /// No deserialization and no payload inspection.
    Transfer,
    /// Routed on metadata.
    Light,
    /// Interpreted through Content, Contract and Path.
    Context,
}

/// `failover`: where a Send Port goes when its active Send Location has
/// failed its retries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Failover {
    /// The next Send Location, in the order configured.
    Next,
    /// None: the Journey fails.
    None,
}

/// `on_failure`: what an ordered sequence does when one of it fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OnFailure {
    /// The sequence waits behind it.
    Block,
    /// It is set aside and the sequence continues.
    Skip,
}

/// A Receive Location as designed: its name, the Receive Port it belongs
/// to, and what it states of its interaction and depth. The binding gives
/// it its transport, address and node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignedReceiveLocation {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receive_port: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction: Option<Interaction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<Depth>,
}

/// `retry`: how often the active Send Location is tried again, and how long
/// after each failure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retry {
    pub attempts: u32,
    /// A duration: `250ms`, `5s`, `1m`, `1h`.
    pub backoff: String,
}

/// A Send Port as designed, with its policy. The binding gives it the Send
/// Location it leaves by.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignedSendPort {
    pub name: String,
    /// Its Send Locations, by name, tried in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub send_locations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<Retry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failover: Option<Failover>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_style: Option<ExecutionStyle>,
    /// What a sequence is ordered by, for example `party`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<OnFailure>,
}

/// What is wrong with the Receive Locations of `application` at its
/// `receive_ports`, one sentence each.
pub(crate) fn receive_problems(
    application: &str,
    receive_ports: &[DesignedElement],
    locations: &[DesignedReceiveLocation],
    problems: &mut Vec<String>,
) {
    for location in locations {
        let name = &location.name;
        match &location.receive_port {
            None => problems.push(format!(
                "the Receive Location '{name}' of '{application}' names no receive_port"
            )),
            Some(port) if !receive_ports.iter().any(|declared| &declared.name == port) => {
                problems.push(format!(
                    "the Receive Location '{name}' of '{application}' names the Receive Port \
                     '{port}', which the Application does not declare"
                ));
            }
            Some(_) => {}
        }
        if location.interaction.is_none() {
            problems.push(format!(
                "the Receive Location '{name}' of '{application}' states no interaction \
                 (composite, data-transfer or batch-load)"
            ));
        }
        if location.depth.is_none() {
            problems.push(format!(
                "the Receive Location '{name}' of '{application}' states no depth \
                 (transfer, light or context)"
            ));
        }
    }
}

/// What is wrong with the policy of each of `application`'s Send Ports.
pub(crate) fn send_problems(
    application: &str,
    ports: &[DesignedSendPort],
    problems: &mut Vec<String>,
) {
    for port in ports {
        let name = &port.name;
        if port.execution_style == Some(ExecutionStyle::Sequential) && port.on_failure.is_none() {
            problems.push(format!(
                "the Sequential Send Port '{name}' of '{application}' states no on_failure \
                 (block or skip)"
            ));
        }
        if let Some(retry) = &port.retry {
            if retry.attempts == 0 {
                problems.push(format!(
                    "the Send Port '{name}' of '{application}' retries 0 attempts; leave retry \
                     out to try once"
                ));
            }
            if let Err(error) = xcore::settings::duration(&retry.backoff) {
                problems.push(format!(
                    "the retry backoff of the Send Port '{name}' of '{application}': {error}"
                ));
            }
        }
        if port
            .order_key
            .as_deref()
            .is_some_and(|key| key.trim().is_empty())
        {
            problems.push(format!(
                "the Send Port '{name}' of '{application}' names an empty order_key"
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for location in &port.send_locations {
            if location.trim().is_empty() || !seen.insert(location.as_str()) {
                problems.push(format!(
                    "the Send Port '{name}' of '{application}' names the Send Location \
                     '{location}' empty or twice"
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parse_application;

    const INVOICES: &str = r#"name = "Invoices"

[[receive_ports]]
name = "Invoices"

[[receive_locations]]
name = "InvoicesHttp"
receive_port = "Invoices"
interaction = "data-transfer"
depth = "light"

[[send_ports]]
name = "ErpOut"
send_locations = ["ErpPrimary", "ErpBackup"]
retry = { attempts = 3, backoff = "5s" }
failover = "next"
execution_style = "sequential"
order_key = "party"
on_failure = "block"
"#;

    #[test]
    fn the_ports_and_their_settings_read_as_the_record_writes_them() {
        let application = parse_application(INVOICES).expect("reads");
        assert!(
            application.problems().is_empty(),
            "{:?}",
            application.problems()
        );
        let location = &application.receive_locations[0];
        assert_eq!(location.receive_port.as_deref(), Some("Invoices"));
        assert_eq!(location.interaction, Some(super::Interaction::DataTransfer));
        assert_eq!(location.depth, Some(super::Depth::Light));
        let port = &application.send_ports[0];
        assert_eq!(port.send_locations, ["ErpPrimary", "ErpBackup"]);
        assert_eq!(port.retry.as_ref().map(|r| r.attempts), Some(3));
        assert_eq!(port.failover, Some(super::Failover::Next));
        assert_eq!(port.on_failure, Some(super::OnFailure::Block));
    }

    #[test]
    fn a_location_without_its_port_or_its_statements_is_refused() {
        let problems = parse_application(
            &INVOICES
                .replace("receive_port = \"Invoices\"\n", "")
                .replace("depth = \"light\"\n", ""),
        )
        .expect("reads")
        .problems();
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(
            problems[0].contains("names no receive_port"),
            "{problems:?}"
        );
        assert!(problems[1].contains("states no depth"), "{problems:?}");

        let elsewhere = parse_application(
            &INVOICES.replace("receive_port = \"Invoices\"", "receive_port = \"Orders\""),
        )
        .expect("reads")
        .problems();
        assert!(elsewhere[0].contains("'Orders'"), "{elsewhere:?}");
    }

    #[test]
    fn a_sequential_send_port_without_on_failure_and_a_bad_backoff_are_refused() {
        let problems = parse_application(
            &INVOICES
                .replace("on_failure = \"block\"\n", "")
                .replace("\"5s\"", "\"soon\""),
        )
        .expect("reads")
        .problems();
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].contains("states no on_failure"), "{problems:?}");
        assert!(problems[1].contains("backoff"), "{problems:?}");
    }

    #[test]
    fn a_word_a_key_does_not_have_is_refused_as_it_reads() {
        for (from, to) in [
            ("\"data-transfer\"", "\"streaming\""),
            ("\"next\"", "\"random\""),
            ("\"block\"", "\"retry\""),
        ] {
            assert!(
                parse_application(&INVOICES.replace(from, to)).is_err(),
                "{to}"
            );
        }
    }
}
