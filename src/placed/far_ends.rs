//! Where each Location placed is bound when the Parties' far ends are
//! machines of their own: the owner's node plan of 2026-11-22, *the external
//! TX's can shoot at any internal RX, so can the internal TX shoot at any
//! external RX*. A Receive Location listens on its own node, over HTTP, so
//! any sending far end pushes to it; or, where its environment says none
//! listens, it fetches from the far end of its turn. A Send Port puts on the
//! far end of its turn. The turn is one rule, [`far_end_of`], which the
//! System Tests drive their far ends by on one computer, and the Systems Tests on machines.

use super::{Binding, Placed, Placement, far_end};

/// The port the first Receive Location listens on where the far ends push to
/// it; each next one placed listens on the next.
pub const LISTENING_PORT: u16 = 7480;

/// Which of `count` far ends the `at`-th placement of its kind goes to:
/// round robin in the order they are placed, so each far end is reached by
/// every node that sends where there are enough Send Ports.
#[must_use]
pub const fn far_end_of(at: usize, count: usize) -> usize {
    match at.checked_rem(count) {
        Some(turn) => turn,
        None => 0,
    }
}

/// Each Receive Location and Send Port of `placements`, bound over
/// `far_ends`: a Receive Location listening over HTTP on its node, on the
/// next port from [`LISTENING_PORT`], where `listening`, else fetching
/// from the far end of its turn; a Send Port putting on the far end of its
/// turn, a user of its own there ([`far_end`]). A Work Process is bound by
/// nothing and is left out.
#[must_use]
pub fn bound(
    placements: &[Placement],
    far_ends: &[String],
    listening: bool,
) -> Vec<(Placement, Binding)> {
    let (mut receive, mut send) = (0_usize, 0_usize);
    let mut bound = Vec::new();
    for placement in placements {
        let binding = match placement.placed {
            Placed::Receive if listening => {
                let port = u16::try_from(receive)
                    .ok()
                    .and_then(|at| LISTENING_PORT.checked_add(at))
                    .unwrap_or(LISTENING_PORT);
                receive += 1;
                Binding {
                    address: format!("http://0.0.0.0:{port}/{}", placement.folded()),
                    settings: Vec::new(),
                    secret: None,
                    accepting: true,
                }
            }
            Placed::Receive | Placed::Send => {
                let turn = if placement.placed == Placed::Receive {
                    &mut receive
                } else {
                    &mut send
                };
                let host = far_ends
                    .get(far_end_of(*turn, far_ends.len()))
                    .map_or("", String::as_str);
                *turn += 1;
                far_end(host)(placement)
            }
            Placed::Process => continue,
        };
        bound.push((placement.clone(), binding));
    }
    bound
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(placed: Placed, name: &str, node: &str) -> Placement {
        Placement {
            placed,
            application: "A".to_string(),
            name: name.to_string(),
            node: node.to_string(),
        }
    }

    #[test]
    fn the_turn_goes_round_the_far_ends_in_the_order_placed() {
        let given: Vec<usize> = (0..7).map(|at| far_end_of(at, 3)).collect();
        assert_eq!(given, [0, 1, 2, 0, 1, 2, 0]);
        assert_eq!(far_end_of(4, 0), 0, "none is no turn");
    }

    /// Listening, each Receive Location listens on its node on a port of
    /// its own; each Send Port puts on the receiving far end of its turn.
    #[test]
    fn a_receive_location_listens_on_its_own_port_and_a_send_port_puts_on_its_turns() {
        let cluster = crate::fixture::test_cluster();
        let node = |place: usize| cluster.node(place).name.as_str();
        let placements = [
            placement(Placed::Receive, "Orders in", node(0)),
            placement(Placed::Send, "Orders out", node(1)),
            placement(Placed::Receive, "Invoices in", node(2)),
            placement(Placed::Send, "Invoices out", node(3)),
            placement(Placed::Process, "Approval", node(4)),
        ];
        let far_ends = ["receiver-a".to_string(), "receiver-b".to_string()];
        let bound = bound(&placements, &far_ends, true);
        let addresses: Vec<&str> = bound.iter().map(|(_, one)| one.address.as_str()).collect();
        assert_eq!(
            addresses,
            [
                "http://0.0.0.0:7480/orders-in",
                "sftp://orders-out@receiver-a:7422",
                "http://0.0.0.0:7481/invoices-in",
                "sftp://invoices-out@receiver-b:7422",
            ]
        );
        assert!(bound[0].1.accepting && bound[0].1.secret.is_none());
        assert!(!bound[1].1.accepting && bound[1].1.secret.is_some());
        // Not listening, a Receive Location fetches from the far end of its
        // turn as a Send Port puts on its own: one far end serves them all.
        let fetching = super::bound(&placements, &far_ends[..1], false);
        assert_eq!(fetching[0].1.address, "sftp://orders-in@receiver-a:7422");
        assert!(fetching[0].1.accepting);
    }
}
