//! `[service]`: the cluster and node a document configures, the short name
//! the surfaces show for the cluster, where its data directory is, and
//! where in it the node keeps its own files.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// `[service]`: the cluster and node this document configures. A key it
/// does not read is refused, never ignored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceConfiguration {
    pub name: String,
    pub cluster_name: String,
    pub node_name: String,
    /// The cluster's short name, what every operator surface shows for the
    /// cluster, as the cluster's file declares it in its own `[service]`
    /// and the slice gives every node ([`ShortName`]; ADR-0028, amendment
    /// 2026-11-28). Absent, a surface shows the cluster's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_name: Option<ShortName>,
    /// The node's data directory: where its embedded Storage node, its keys
    /// and the orders an operator leaves for it are kept, the keys unless
    /// `[store]` says otherwise. Relative to the configuration file; absent, `../data`,
    /// which in the installed layout (ADR-0015 clause 10) is the `data`
    /// beside the `config` the file is in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

/// Where a node's data directory is when its configuration does not say.
pub const DEFAULT_DATA: &str = "../data";

impl ServiceConfiguration {
    /// What `[service]`'s keys mean, in the words an editor's hover and a
    /// designer show beside the key (`xmip-core-operate`'s `design::keys`).
    pub const MEANINGS: [(&'static str, &'static str); 1] = [(
        "short_name",
        "The cluster's short name, which every operator surface shows for the cluster; \
         its tooltip carries the cluster's name and this file. One to twelve letters, \
         digits or hyphens, the first a letter.",
    )];

    /// The node's data directory, for the configuration file at
    /// `configuration`: written in the platform's own separators, a `.`
    /// left out — `config\..\data\storage` never `config\../data\storage`
    /// — and, on Windows, a `..` taken back as the system itself resolves
    /// a path, by its words: `data\storage`. Elsewhere a `..` stays, since
    /// a directory before it may be a link.
    #[must_use]
    pub fn data_directory(&self, configuration: &Path) -> PathBuf {
        data_directory(self.data.as_deref(), configuration)
    }
}

/// The data directory `data` names, [`DEFAULT_DATA`] where it names none,
/// for the configuration file at `configuration`: as
/// [`ServiceConfiguration::data_directory`] says.
#[must_use]
pub fn data_directory(data: Option<&str>, configuration: &Path) -> PathBuf {
    let base = configuration.parent().unwrap_or_else(|| Path::new(""));
    let joined = base.join(data.unwrap_or(DEFAULT_DATA));
    let mut clean = PathBuf::new();
    for part in joined.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
                if cfg!(windows)
                    && matches!(
                        clean.components().next_back(),
                        Some(std::path::Component::Normal(_))
                    ) =>
            {
                clean.pop();
            }
            other => clean.push(other),
        }
    }
    clean
}

/// `<stem>.<kept>` in the default data directory of the configuration at
/// `path`, `../data` from its folder ([`data_directory`]): where the node
/// keeps a file of its own — its pin and its active configuration
/// (`crate::signed`), the number of its latest start (`crate::start`) —
/// found from the configuration's place alone, never from what a slice
/// says.
pub(crate) fn kept_at(path: &Path, kept: &str) -> PathBuf {
    let stem = path
        .file_stem()
        .map_or_else(|| "node".into(), |stem| stem.to_string_lossy());
    data_directory(None, path).join(format!("{stem}.{kept}"))
}

/// A cluster's short name, `[service] short_name`: what the operator
/// surfaces show where they show the cluster (the owner, 2026-11-28: *add a
/// short name to be shown*; *The short name should be declared in the xmip
/// file*). One to [`ShortName::LONGEST`] letters, digits or hyphens, the
/// first a letter: a word a status bar holds. The reader refuses any other
/// in words; nothing reads meaning from it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ShortName(String);

impl ShortName {
    /// The most characters a short name holds.
    pub const LONGEST: usize = 12;

    /// The short name as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ShortName {
    type Error = String;

    fn try_from(given: String) -> Result<Self, Self::Error> {
        let mut characters = given.chars();
        let begins = characters
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic());
        let rest = characters.all(|one| one.is_ascii_alphanumeric() || one == '-');
        if begins && rest && given.len() <= Self::LONGEST {
            Ok(Self(given))
        } else {
            Err(format!(
                "REFUSED: the short name '{given}' is not one to {} letters, digits or \
                 hyphens beginning with a letter",
                Self::LONGEST
            ))
        }
    }
}

impl From<ShortName> for String {
    fn from(short: ShortName) -> Self {
        short.0
    }
}

impl fmt::Display for ShortName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The short name the document `text` declares, `[service] short_name`: a
/// cluster's file, which declares it, and each node's slice, which the
/// slicing gave it, alike; `None` where it declares none.
///
/// # Errors
/// When the text is not TOML, or its short name is no [`ShortName`].
pub fn short_name(text: &str) -> Result<Option<ShortName>, String> {
    let table = crate::document::read(text)?;
    match table
        .get("service")
        .and_then(|service| service.get("short_name"))
    {
        None => Ok(None),
        Some(toml::Value::String(said)) => ShortName::try_from(said.clone()).map(Some),
        Some(_) => Err("[service] short_name is a string".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_name_is_a_word_and_anything_else_is_refused_in_words() {
        let longest = format!("A{}", "1".repeat(ShortName::LONGEST - 1));
        for good in ["X", "Ab-2", longest.as_str()] {
            assert_eq!(
                ShortName::try_from(good.to_string()).map(String::from),
                Ok(good.into())
            );
        }
        for bad in ["", "1A", "-A", "A B", "Å", &format!("{longest}2")] {
            let refused = ShortName::try_from(bad.to_string()).expect_err(bad);
            assert!(refused.starts_with("REFUSED: the short name"), "{refused}");
        }
    }

    #[test]
    fn service_reads_its_short_name_and_refuses_a_key_it_does_not_read() {
        let head = "name = \"n\"\ncluster_name = \"c\"\nnode_name = \"d\"\n";
        let read: ServiceConfiguration =
            toml::from_str(&format!("{head}short_name = \"Ab\"\n")).expect("reads");
        assert_eq!(read.short_name.as_ref().map(ShortName::as_str), Some("Ab"));
        let written = toml::to_string(&read).expect("writes");
        let back: ServiceConfiguration = toml::from_str(&written).expect("reads back");
        assert_eq!(back, read);
        let none: ServiceConfiguration = toml::from_str(head).expect("reads");
        assert_eq!(none.short_name, None);

        let wrong =
            toml::from_str::<ServiceConfiguration>(&format!("{head}short_name = \"a b\"\n"))
                .expect_err("no word");
        assert!(
            wrong.to_string().contains("REFUSED: the short name 'a b'"),
            "{wrong}"
        );
        let unknown =
            toml::from_str::<ServiceConfiguration>(&format!("{head}shortname = \"Ab\"\n"))
                .expect_err("unknown");
        assert!(
            unknown.to_string().contains("unknown field `shortname`"),
            "{unknown}"
        );
    }

    /// The cluster's file declares it once, and every node's slice carries
    /// it, signed with the rest of the slice.
    #[test]
    fn the_file_declares_the_short_name_and_every_slice_carries_it() {
        let cluster = crate::fixture::test_cluster();
        let [one, two] = [0, 1].map(|at| cluster.node(at).name.clone());
        let text = format!("[service]\nshort_name = \"Ab\"\n[nodes.{one}]\n[nodes.{two}]\n");
        let said = |text: &str| short_name(text).expect("reads").map(String::from);
        assert_eq!(said(&text), Some("Ab".to_string()));
        for node in [&one, &two] {
            let sliced = crate::cluster::slice_named(&text, node, &cluster.name).expect("slices");
            let document = crate::parse_toml(&sliced).expect("reads as a node's");
            assert_eq!(
                document.service.short_name.map(String::from),
                Some("Ab".into())
            );
            assert_eq!(said(&sliced), Some("Ab".to_string()));
        }
        assert!(short_name("[service]\nshort_name = 1\n").is_err());
        assert!(short_name("[service]\nshort_name = \"a b\"\n").is_err());
        assert_eq!(said("[service]\n"), None);
    }
}
