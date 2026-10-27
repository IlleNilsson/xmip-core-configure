//! Correlation sets taken in crossing order: refused (the owner,
//! 2026-10-27: *Validation refuses a configuration where two paths could
//! take correlation sets in crossing order. Sets are taken in path order*).
//!
//! A Journey takes its places in path order — Receive Location, Receive
//! Port, Work Process, Send Port, Send Location — and holds its Stream's
//! end to end while it waits for the ones after. Where one path takes set
//! `A` before set `B` and another takes `B` before `A`, Journeys of equal
//! keys could each hold what the other waits for. A Subscription's filter
//! may match any Message, so every receive point is taken to reach every
//! destination: each path is a receive point's sets, then a Work Process's,
//! then a Send Port's and its Send Locations'. Every pair of sets one path
//! takes in order is an edge; a cycle among them is refused in words,
//! naming the sets, the paths and the fix.

use std::collections::BTreeMap;

use crate::Bound;

/// One path's sets in the order it takes them, and how a person names it.
struct Path {
    named: String,
    sets: Vec<String>,
}

/// Every path through `bound`, its Work Processes `work_processes` (each
/// a name and the set it names) among them.
fn paths(bound: &Bound, work_processes: &[(String, String)]) -> Vec<Path> {
    let mut receives: Vec<(String, Vec<String>)> = Vec::new();
    for port in &bound.receive_ports {
        for location in &port.receive_locations {
            let sets = [&location.correlation_set, &port.correlation_set]
                .into_iter()
                .flatten()
                .cloned()
                .collect();
            receives.push((format!("the Receive Location '{}'", location.name), sets));
        }
    }
    let mut sends: Vec<(String, Vec<String>)> = Vec::new();
    for port in &bound.designed_send_ports {
        let locations: Vec<&str> = if port.send_locations.is_empty() {
            vec![port.name.as_str()]
        } else {
            port.send_locations.iter().map(String::as_str).collect()
        };
        let mut sets: Vec<String> = port.correlation_set.iter().cloned().collect();
        for name in locations {
            sets.extend(
                bound
                    .send_location_sets
                    .iter()
                    .filter(|(location, _)| location == name)
                    .map(|(_, set)| set.clone()),
            );
        }
        sends.push((format!("the Send Port '{}'", port.name), sets));
    }
    let mut middles: Vec<(String, Vec<String>)> = vec![(String::new(), Vec::new())];
    middles.extend(work_processes.iter().map(|(name, set)| {
        (
            format!(" through the Work Process '{name}'"),
            vec![set.clone()],
        )
    }));
    let mut every = Vec::new();
    for (receive, first) in &receives {
        for (middle, between) in &middles {
            for (send, last) in &sends {
                every.push(Path {
                    named: format!("{receive}{middle} to {send}"),
                    sets: [first.clone(), between.clone(), last.clone()].concat(),
                });
            }
        }
    }
    every
}

/// Where sets are taken in crossing order among `bound`'s paths: one
/// sentence naming the sets, a path taking each order, and the fix.
#[must_use]
pub fn crossing(bound: &Bound, work_processes: &[(String, String)]) -> Vec<String> {
    // Each set taken before another, and the first path that does.
    let mut before: BTreeMap<(String, String), String> = BTreeMap::new();
    for path in paths(bound, work_processes) {
        for (at, earlier) in path.sets.iter().enumerate() {
            for later in &path.sets[at + 1..] {
                if earlier != later {
                    before
                        .entry((earlier.clone(), later.clone()))
                        .or_insert_with(|| path.named.clone());
                }
            }
        }
    }
    // Taken before by way of other sets too: a longer cycle crosses as well.
    let sets: Vec<&String> = before
        .keys()
        .flat_map(|(a, b)| [a, b])
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut reach: BTreeMap<(&String, &String), bool> = BTreeMap::new();
    for (a, b) in before.keys() {
        reach.insert((a, b), true);
    }
    for via in &sets {
        for from in &sets {
            for to in &sets {
                if reach.contains_key(&(*from, *via)) && reach.contains_key(&(*via, *to)) {
                    reach.insert((*from, *to), true);
                }
            }
        }
    }
    let said = |a: &String, b: &String| {
        before.get(&(a.clone(), b.clone())).map_or_else(
            || format!("paths through other sets take '{a}' before '{b}'"),
            |path| format!("{path} takes '{a}' before '{b}'"),
        )
    };
    let mut problems = Vec::new();
    for (at, first) in sets.iter().enumerate() {
        for second in &sets[at + 1..] {
            if reach.contains_key(&(*first, *second)) && reach.contains_key(&(*second, *first)) {
                problems.push(format!(
                    "the correlation sets '{first}' and '{second}' are taken in crossing order: \
                     {}, and {}, so Journeys of equal keys could each wait for the other for \
                     ever; take '{first}' before '{second}' on both paths",
                    said(first, second),
                    said(second, first)
                ));
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::crossing;
    use crate::{Bound, BoundReceivePort, DesignedSendPort};

    /// A Receive Location keyed by `receive` at its Port, and a Send Port
    /// keyed by `send`, each by set name.
    fn path(bound: &mut Bound, (location, receive): (&str, &str), (port, send): (&str, &str)) {
        let receiving: BoundReceivePort = toml::from_str(&format!(
            "application = \"Orders\"\nname = \"In{location}\"\n\
             [[receive_locations]]\nname = \"{location}\"\ncorrelation_set = \"{receive}\"\n"
        ))
        .expect("a Receive Port");
        bound.receive_ports.push(receiving);
        let sending: DesignedSendPort = toml::from_str(&format!(
            "name = \"{port}\"\ncorrelation_set = \"{send}\"\n"
        ))
        .expect("a Send Port");
        bound.designed_send_ports.push(sending);
    }

    #[test]
    fn two_paths_taking_two_sets_in_crossing_order_are_refused_naming_the_fix() {
        let mut bound = Bound::default();
        path(&mut bound, ("FromA", "PerClient"), ("ToA", "PerOrder"));
        path(&mut bound, ("FromB", "PerOrder"), ("ToB", "PerClient"));
        let refused = crossing(&bound, &[]);
        assert_eq!(refused.len(), 1, "{refused:?}");
        for words in [
            "'PerClient' and 'PerOrder' are taken in crossing order",
            "the Receive Location 'FromA' to the Send Port 'ToA' takes 'PerClient' before",
            "the Receive Location 'FromB' to the Send Port 'ToB' takes 'PerOrder' before",
            "take 'PerClient' before 'PerOrder' on both paths",
        ] {
            assert!(refused[0].contains(words), "{words}: {}", refused[0]);
        }
    }

    #[test]
    fn paths_taking_their_sets_in_one_order_are_sound() {
        let mut bound = Bound::default();
        path(&mut bound, ("FromA", "PerClient"), ("ToA", "PerOrder"));
        path(&mut bound, ("FromB", "PerClient"), ("ToB", "PerOrder"));
        assert_eq!(crossing(&bound, &[]), [] as [String; 0]);
    }

    #[test]
    fn a_longer_cycle_through_a_work_process_crosses_too() {
        let mut bound = Bound::default();
        path(&mut bound, ("FromA", "A"), ("ToA", "B"));
        let processes = [("Approval".to_string(), "C".to_string())];
        // A before C (through the Work Process) and C before A nowhere yet.
        assert_eq!(crossing(&bound, &processes), [] as [String; 0]);
        path(&mut bound, ("FromB", "B"), ("ToB", "A"));
        assert!(!crossing(&bound, &processes).is_empty());
    }
}
