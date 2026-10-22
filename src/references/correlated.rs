//! A point's `correlation_set`: a Receive Location, Receive Port, Work
//! Process or Send Port of an Xmip Application names a Correlation Set of
//! the cluster's `[[correlation_sets]]` (`crate::correlation`).

use toml::{Table, Value};

use super::{Reference, named, slot};
use crate::section::SECTIONS;

/// The lists of an Application whose entries may name a correlation set.
const POINTS: [&str; 4] = [
    "receive_locations",
    "receive_ports",
    "work_processes",
    "send_ports",
];

/// Where the cluster's correlation sets are declared.
const SETS: &str = "correlation_sets";

/// Each point's `correlation_set` in the cluster's file `cluster`, written
/// or not.
pub(super) fn correlated(cluster: &Table, found: &mut Vec<Reference>) {
    let sets: Vec<String> = named(Some(cluster), SETS).map(|(name, _)| name).collect();
    for (application, own) in named(Some(cluster), SECTIONS) {
        for list in POINTS {
            for (name, entry) in named(Some(own), list) {
                let written = entry.get("correlation_set").and_then(Value::as_str);
                found.push(slot(
                    super::path(&[SECTIONS, &application, list, &name]),
                    &["correlation_set"],
                    None,
                    written,
                    "Correlation Set",
                    &[SETS],
                    &sets,
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::references;

    #[test]
    fn a_points_correlation_set_names_the_clusters_declaration() {
        let found = references(
            "[[correlation_sets]]\nname = \"PerClient\"\nkeys = [\"context:Client\"]\n\
             [[xmip_applications]]\nname = \"Orders\"\n\
             [[xmip_applications.receive_locations]]\nname = \"In\"\n\
             correlation_set = \"PerClient\"\n\
             [[xmip_applications.send_ports]]\nname = \"Out\"\n",
        )
        .expect("reads");
        let named = |entry: &[&str]| {
            found
                .iter()
                .find(|r| r.holder == entry && r.key == ["correlation_set"])
                .expect("the point's correlation set")
        };
        let location = named(&["xmip_applications", "Orders", "receive_locations", "In"]);
        assert_eq!(location.name.as_deref(), Some("PerClient"));
        assert_eq!(
            location.target,
            Some(vec!["correlation_sets".to_string(), "PerClient".to_string()])
        );
        let port = named(&["xmip_applications", "Orders", "send_ports", "Out"]);
        assert_eq!(port.name, None);
        assert_eq!(port.choices, ["PerClient"]);
    }
}
