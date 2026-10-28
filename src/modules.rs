//! `[[modules]]`: a module the node loads, its manifest and whether it
//! starts; and what a module's compiled units declare a call to them may
//! give (ADR-0066, amendment 2026-10-22).
//!
//! A unit's settings are declared in its manifest as data
//! (`abi::UnitSetting`), and read here into `xmip-core`'s one declaration
//! shape, so a call's settings are held to them by the same reading that
//! holds a Location's settings to its technology's ([`declared`]).

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::Duration;

use abi::{ModuleManifest, ModuleUnit, UnitCapability, UnitDefault, UnitSetting};
use node::Stage;
use serde::{Deserialize, Serialize};
use xcore::settings::{Applies, Fixed, Kind, Presence, Setting, Settings, duration};

/// `[[modules]]`: a module the node loads, its manifest, and whether it starts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleConfiguration {
    pub name: String,
    pub start: bool,
    pub manifest: ModuleManifest,
}

/// Every `[[modules]]` entry a cluster's file declares that reads — the
/// cluster's and each node's — with the path of the list holding it, and
/// the standard module, built in, once, with no path: nothing in the file
/// declares it (`crate::standard`).
#[must_use]
pub fn declared_in(table: &toml::Table) -> Vec<(Vec<String>, ModuleConfiguration)> {
    let nodes = table
        .get(crate::cluster::NODES)
        .and_then(toml::Value::as_table)
        .into_iter()
        .flatten()
        .filter_map(|(name, own)| {
            let path = vec![crate::cluster::NODES.to_string(), name.clone()];
            own.as_table().map(|own| (path, own))
        });
    std::iter::once((Vec::new(), table))
        .chain(nodes)
        .flat_map(|(at, top)| {
            let listed = top.get("modules").and_then(toml::Value::as_array);
            listed.into_iter().flatten().filter_map(move |module| {
                let module = ModuleConfiguration::deserialize(module.clone()).ok()?;
                Some(([at.clone(), vec!["modules".to_string()]].concat(), module))
            })
        })
        .filter(|(_, module)| module.name != crate::standard::MODULE)
        .chain(std::iter::once((
            Vec::new(),
            crate::standard::configuration(),
        )))
        .collect()
}

impl ModuleConfiguration {
    /// The unit called `name` its manifest declares.
    #[must_use]
    pub fn unit(&self, name: &str) -> Option<&ModuleUnit> {
        self.manifest.units.iter().find(|unit| unit.name == name)
    }

    /// What is wrong with the module and the units its manifest declares,
    /// one sentence each: the module or a unit not declared to work offline
    /// (the owner, 2026-10-22: *We can only use standards, code and
    /// compiled units that explicitly work offline!*), a unit named twice,
    /// its stages ([`stage_problems`]), and a setting not declared as one
    /// reads.
    #[must_use]
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if !self.manifest.offline {
            problems.push(format!(
                "the module '{}' does not declare that it works offline: Xmip uses only \
                 what says offline = true",
                self.name
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for unit in &self.manifest.units {
            if !unit.offline {
                problems.push(format!(
                    "the module '{}' declares the unit '{}', which does not declare that it \
                     works offline: Xmip uses only what says offline = true",
                    self.name, unit.name
                ));
            }
            problems.extend(stage_problems(&self.name, unit));
            if !seen.insert(unit.name.as_str()) {
                problems.push(format!(
                    "the module '{}' declares the unit '{}' twice",
                    self.name, unit.name
                ));
            }
            if let Err(problem) = declared(&self.name, unit) {
                problems.push(problem);
            }
        }
        problems
    }
}

/// Whether `unit` may be called at `stage`: its stages name it.
#[must_use]
pub fn serves(unit: &ModuleUnit, stage: Stage) -> bool {
    unit.stages.iter().any(|word| word == stage.name())
}

/// The stages a unit of `capability` may serve, where they are fixed by
/// definition: Promote on receive and Demote on send (the owner,
/// 2026-10-22), Debatch on receive and in a Work Process (the owner,
/// 2026-10-27).
#[must_use]
pub const fn fixed_stages(capability: UnitCapability) -> Option<&'static [Stage]> {
    match capability {
        UnitCapability::Promote => Some(&[Stage::Receive]),
        UnitCapability::Demote => Some(&[Stage::Send]),
        UnitCapability::Debatch => Some(&[Stage::Receive, Stage::Process]),
        _ => None,
    }
}

/// What is wrong with the stages `unit` of `module` lists: none, a word no
/// stage is called, one listed twice, and a unit whose stages are fixed
/// listing another.
fn stage_problems(module: &str, unit: &ModuleUnit) -> Vec<String> {
    let what = format!(
        "the module '{module}' declares the {} unit '{}'",
        unit.capability.word(),
        unit.name
    );
    let mut problems = Vec::new();
    if unit.stages.is_empty() {
        problems.push(format!("{what} with no stage: {}", Stage::WORDS.join(", ")));
    }
    for (at, word) in unit.stages.iter().enumerate() {
        match Stage::named(word) {
            None => problems.push(format!(
                "{what} for the stage '{word}', which is none of {}",
                Stage::WORDS.join(", ")
            )),
            Some(_) if unit.stages[..at].contains(word) => {
                problems.push(format!("{what} with the stage '{word}' twice"));
            }
            Some(stage) => {
                if let Some(fixed) = fixed_stages(unit.capability)
                    && !fixed.contains(&stage)
                {
                    let words: Vec<String> = fixed
                        .iter()
                        .map(|one| format!("'{}'", one.name()))
                        .collect();
                    problems.push(format!(
                        "{what} for the stage '{word}': a {} unit serves {} alone",
                        unit.capability.word(),
                        words.join(" and ")
                    ));
                }
            }
        }
    }
    problems
}

/// What a call to `unit` of `module` may give, in `xmip-core`'s declaration
/// shape: every setting applying wherever the call stands.
///
/// The shape is `'static`, as a technology's `const` is, so each distinct
/// declaration is made once and kept for the life of the process: what is
/// kept grows with the distinct declarations read, never with how often one
/// is read.
///
/// # Errors
/// A setting whose kind, presence or default is not one the shape has.
pub fn declared(module: &str, unit: &ModuleUnit) -> Result<&'static Settings, String> {
    static MADE: OnceLock<Mutex<BTreeMap<String, &'static Settings>>> = OnceLock::new();
    let key = format!("{module}\u{0}{unit:?}");
    let made = MADE.get_or_init(Mutex::default);
    if let Some(found) = made
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
    {
        return Ok(found);
    }
    let technology = format!("{module} unit {}", unit.name);
    let settings = unit
        .settings
        .iter()
        .map(|setting| shaped(&technology, setting))
        .collect::<Result<Vec<_>, _>>()?;
    let shaped: &'static Settings = Box::leak(Box::new(Settings {
        technology: kept(technology),
        settings: Box::leak(settings.into_boxed_slice()),
    }));
    made.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key, shaped);
    Ok(shaped)
}

fn kept(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

/// One declared setting in the shape.
fn shaped(technology: &str, setting: &UnitSetting) -> Result<Setting, String> {
    let refused = |what: &str| {
        format!(
            "{technology} declares the setting '{}' {what}",
            setting.name
        )
    };
    let kind = match setting.kind.as_str() {
        "text" => Kind::Text,
        "integer" => Kind::Integer {
            minimum: setting.minimum.unwrap_or(i64::MIN),
            maximum: setting.maximum.unwrap_or(i64::MAX),
        },
        "boolean" => Kind::Boolean,
        "duration" => Kind::Duration,
        "address" => Kind::Address,
        "secret" => Kind::Secret,
        "choice" if !setting.choices.is_empty() => Kind::Choice {
            choices: Box::leak(
                setting
                    .choices
                    .iter()
                    .map(|choice| kept(choice.clone()))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
        },
        "choice" => return Err(refused("a choice with no words")),
        other => {
            return Err(refused(&format!(
                "of the kind '{other}', which is none of text, integer, boolean, duration, \
                 address, secret or choice"
            )));
        }
    };
    let presence = match (setting.presence.as_str(), &setting.default) {
        ("required", None) => Presence::Required,
        ("optional", None) => Presence::Optional,
        ("default", Some(default)) => {
            Presence::Default(fixed(kind, default).map_err(|e| refused(&e))?)
        }
        ("default", None) => return Err(refused("with a default and gives none")),
        ("required" | "optional", Some(_)) => {
            return Err(refused("with a default its presence does not say"));
        }
        (other, _) => {
            return Err(refused(&format!(
                "'{other}', which is none of required, optional or default"
            )));
        }
    };
    Ok(Setting {
        name: kept(setting.name.clone()),
        kind,
        presence,
        meaning: kept(setting.meaning.clone()),
        applies: Applies::Both,
    })
}

/// A default as the shape holds it, a duration as one.
fn fixed(kind: Kind, default: &UnitDefault) -> Result<Fixed, String> {
    match (kind, default) {
        (Kind::Duration, UnitDefault::Text(text)) => {
            duration(text).map(|held: Duration| Fixed::Duration(held))
        }
        (_, UnitDefault::Text(text)) => Ok(Fixed::Text(kept(text.clone()))),
        (_, UnitDefault::Integer(number)) => Ok(Fixed::Integer(*number)),
        (_, UnitDefault::Boolean(flag)) => Ok(Fixed::Boolean(*flag)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn unit(settings: Vec<UnitSetting>) -> ModuleUnit {
        ModuleUnit {
            name: "Decompress".into(),
            capability: UnitCapability::Prepare,
            stages: vec!["receive".into()],
            offline: true,
            settings,
        }
    }

    #[test]
    fn promote_serves_receive_and_demote_send_alone_and_a_stage_is_a_stage_word() {
        let of = |capability, stages: &[&str]| ModuleUnit {
            name: "u".into(),
            capability,
            stages: stages.iter().map(ToString::to_string).collect(),
            offline: true,
            settings: Vec::new(),
        };
        let wrong = stage_problems("m", &of(UnitCapability::Promote, &["send"]));
        assert!(wrong[0].contains("serves 'receive' alone"), "{wrong:?}");
        assert_eq!(
            stage_problems("m", &of(UnitCapability::Demote, &["send", "receive"])).len(),
            1
        );
        let debatch = of(UnitCapability::Debatch, &["receive", "process"]);
        assert!(stage_problems("m", &debatch).is_empty());
        assert!(serves(&debatch, Stage::Receive) && !serves(&debatch, Stage::Send));
        let sending = stage_problems("m", &of(UnitCapability::Debatch, &["send"]));
        assert!(
            sending[0].contains("serves 'receive' and 'process' alone"),
            "{sending:?}"
        );
        assert!(stage_problems("m", &of(UnitCapability::Validate, &[]))[0].contains("no stage"));
        let unknown = stage_problems("m", &of(UnitCapability::Prepare, &["arrive"]));
        assert!(
            unknown[0].contains("'arrive', which is none of"),
            "{unknown:?}"
        );
    }

    fn setting(kind: &str, presence: &str, default: Option<UnitDefault>) -> UnitSetting {
        UnitSetting {
            name: "format".into(),
            kind: kind.into(),
            minimum: None,
            maximum: None,
            choices: vec!["gzip".into(), "zstd".into()],
            presence: presence.into(),
            default,
            meaning: "How the Stream is compressed.".into(),
        }
    }

    #[test]
    fn a_units_declaration_reads_a_call_as_a_technologys_reads_a_location() {
        let declared = declared(
            "codecs",
            &unit(vec![setting(
                "choice",
                "default",
                Some(UnitDefault::Text("gzip".into())),
            )]),
        )
        .expect("declared");
        let given = [(
            "format".to_string(),
            xcore::settings::Given::Text("lz4".into()),
        )];
        let refused = declared
            .read(Applies::Both, &given)
            .expect_err("not a word");
        assert!(refused.to_string().contains("format"), "{refused}");
        assert!(declared.read(Applies::Both, &[]).is_ok());

        let again = super::declared(
            "codecs",
            &unit(vec![setting(
                "choice",
                "default",
                Some(UnitDefault::Text("gzip".into())),
            )]),
        )
        .expect("declared");
        assert!(std::ptr::eq(declared, again), "made once");
    }

    #[test]
    fn a_setting_declared_as_none_reads_is_refused_in_words() {
        for (kind, presence, default, words) in [
            ("float", "optional", None, "none of text"),
            ("text", "default", None, "gives none"),
            ("text", "sometimes", None, "none of required"),
            (
                "duration",
                "default",
                Some(UnitDefault::Text("soon".into())),
                "format",
            ),
        ] {
            let problem =
                declared("codecs", &unit(vec![setting(kind, presence, default)])).expect_err(kind);
            assert!(problem.contains(words), "{problem}");
        }
    }

    #[test]
    fn a_module_or_unit_not_declared_offline_is_refused_in_words() {
        let module = |offline: bool, unit_offline: bool| {
            let mut unit = unit(Vec::new());
            unit.offline = unit_offline;
            ModuleConfiguration {
                name: "orders".into(),
                start: true,
                manifest: ModuleManifest {
                    identity: abi::ModuleIdentity {
                        name: "orders".into(),
                        version: "1.0.0".into(),
                    },
                    capabilities: Vec::new(),
                    entrypoint: abi::ModuleEntrypoint {
                        library_path: None,
                        executable_path: None,
                        symbol: None,
                    },
                    offline,
                    units: vec![unit],
                },
            }
        };
        assert_eq!(module(true, true).problems(), [] as [String; 0]);
        let found = module(false, false).problems();
        assert!(
            found[0].contains("module 'orders' does not declare"),
            "{found:?}"
        );
        assert!(
            found[1].contains("unit 'Decompress', which does not declare"),
            "{found:?}"
        );
    }
}
