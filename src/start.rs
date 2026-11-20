//! What a node keeps of its starts (ADR-0056, amendment 2026-11-20): the
//! number of its latest start, `<stem>.start.toml` beside its pin in the
//! default data directory, found from the configuration's place alone. Each
//! start is numbered above it, and it is kept before the start's first
//! membership write, so Xmip Storage orders one node's starts as they
//! happened, never as their writes arrive. The form and the place are here;
//! the numbering is the runtime's (`runtime::membership::start`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A node's start file, after its configuration file's stem:
/// `<stem>.start.toml`. A record, not a description of an installation, so
/// it ends in `.toml` (ADR-0031, amendment 2026-11-10), as the pin does.
pub const START: &str = "start.toml";

/// The number of a node's latest start, as it keeps it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    /// The number its latest start took, or the one a later start of the
    /// node registered under, where Xmip Storage refused this node's start
    /// below it: what its next start is numbered above.
    pub start: u64,
}

impl Start {
    /// A start read from the text [`Start::written`] wrote.
    ///
    /// # Errors
    /// A text that is not its one value, in words.
    pub fn read(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|error| format!("it does not read: {}", error.message()))
    }

    /// The start as the node writes it: its value, under a comment saying
    /// what it is.
    #[must_use]
    pub fn written(&self) -> String {
        format!(
            "# What this node numbers its next start above (ADR-0056, amendment\n\
             # 2026-11-20). Written by the node alone; removed, the next start is\n\
             # numbered by the clock.\n\
             start = {}\n",
            self.start
        )
    }
}

/// Where the node whose configuration is at `path` keeps its [`Start`]:
/// `<stem>.start.toml` in the default data directory, beside its pin.
#[must_use]
pub fn start_at(path: &Path) -> PathBuf {
    crate::service::kept_at(path, START)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_start_comes_back_from_its_text_and_is_kept_beside_the_pin() {
        let start = Start { start: u64::MAX };
        assert_eq!(Start::read(&start.written()), Ok(start));
        assert!(
            Start::read("start = 1\nnode = \"x\"\n").is_err(),
            "one value"
        );

        let path = Path::new("base").join("config").join("node.xmip");
        let at = start_at(&path);
        assert_eq!(at.file_name(), Some("node.start.toml".as_ref()));
        assert_eq!(at.parent(), crate::signed::pin_at(&path).parent());
    }
}
