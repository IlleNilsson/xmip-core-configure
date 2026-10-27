//! A point's `correlation`: a Receive Location, Receive Port, Work
//! Process or Send Port of an Xmip Application, or a Send Location — the
//! cluster's own or a binding's — names a Correlation of the cluster's
//! `[[correlations]]` (`crate::correlation`).

use toml::{Table, Value};

use super::{Reference, named, slot};
use crate::section::SECTIONS;

/// The lists of an Application whose entries may name a Correlation.
const POINTS: [&str; 4] = [
    "receive_locations",
    "receive_ports",
    "work_processes",
    "send_ports",
];

/// Where the cluster's Correlations are declared.
const SETS: &str = "correlations";

/// Each point's `correlation` in the cluster's file `cluster`, written
/// or not.
pub(super) fn correlated(cluster: &Table, found: &mut Vec<Reference>) {
    let sets: Vec<String> = named(Some(cluster), SETS).map(|(name, _)| name).collect();
    let mut point = |holder: Vec<String>, entry: &Table| {
        let written = entry.get("correlation").and_then(Value::as_str);
        found.push(slot(
            holder,
            &["correlation"],
            None,
            written,
            "Correlation",
            &[SETS],
            &sets,
        ));
    };
    for (application, own) in named(Some(cluster), SECTIONS) {
        for list in POINTS {
            for (name, entry) in named(Some(own), list) {
                point(super::path(&[SECTIONS, &application, list, &name]), entry);
            }
        }
    }
    // A Send Location names one too: the cluster's own and each binding's
    // (the owner, 2026-10-22: *at any configurable point*).
    for (name, entry) in named(Some(cluster), "send_locations") {
        point(super::path(&["send_locations", &name]), entry);
    }
    for (application, binding) in named(Some(cluster), "applications") {
        for (name, entry) in named(Some(binding), "send_ports") {
            point(
                super::path(&["applications", &application, "send_ports", &name]),
                entry,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::references;

    #[test]
    fn a_points_correlation_names_the_clusters_declaration() {
        let found = references(
            "[[correlations]]\nname = \"PerClient\"\nkeys = [\"context:Client\"]\n\
             [[xmip_applications]]\nname = \"Orders\"\n\
             [[xmip_applications.receive_locations]]\nname = \"In\"\n\
             correlation = \"PerClient\"\n\
             [[xmip_applications.send_ports]]\nname = \"Out\"\n",
        )
        .expect("reads");
        let named = |entry: &[&str]| {
            found
                .iter()
                .find(|r| r.holder == entry && r.key == ["correlation"])
                .expect("the point's Correlation")
        };
        let location = named(&["xmip_applications", "Orders", "receive_locations", "In"]);
        assert_eq!(location.name.as_deref(), Some("PerClient"));
        assert_eq!(
            location.target,
            Some(vec!["correlations".to_string(), "PerClient".to_string()])
        );
        let port = named(&["xmip_applications", "Orders", "send_ports", "Out"]);
        assert_eq!(port.name, None);
        assert_eq!(port.choices, ["PerClient"]);
    }

    #[test]
    fn a_send_location_names_a_correlation_its_own_or_bound() {
        let found = references(
            "[[correlations]]\nname = \"OneAtATime\"\nkeys = [\"send-port\"]\n\
             [[send_locations]]\nname = \"Ftp\"\ntransport = \"t\"\naddress = \"a\"\n\
             correlation = \"OneAtATime\"\n\
             [[applications]]\nname = \"Orders\"\n\
             [[applications.send_ports]]\nname = \"Out\"\nnode = \"n\"\n\
             transport = \"t\"\naddress = \"a\"\n",
        )
        .expect("reads");
        let named = |entry: &[&str]| {
            found
                .iter()
                .find(|r| r.holder == entry && r.key == ["correlation"])
                .expect("the Location's Correlation")
        };
        assert_eq!(
            named(&["send_locations", "Ftp"]).name.as_deref(),
            Some("OneAtATime")
        );
        let bound = named(&["applications", "Orders", "send_ports", "Out"]);
        assert_eq!(bound.name, None);
        assert_eq!(bound.choices, ["OneAtATime"]);
    }
}
