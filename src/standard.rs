//! The standard module, built in (the owner, 2026-10-22): Xmip's own
//! compiled units, which a step names as `module = "xmip"` with no
//! `[[modules]]` entry, its manifest known here rather than written into
//! the configuration. The manifest is written from the declarations each
//! capability gives beside its units' code (`node::unit::Declaration`),
//! read without that code, so configure links no unit; `xmip-core-unit`
//! opens the same declarations where a node runs them (ADR-0066,
//! amendment 2026-10-22).

use abi::unit::{ModuleUnit, UnitCapability, UnitDefault, UnitSetting};
use abi::{ModuleEntrypoint, ModuleIdentity, ModuleManifest};
use node::unit::Declaration;

use crate::modules::ModuleConfiguration;
use xcore::settings::{Fixed, Given, Kind, Presence, Setting};

/// The standard module's name, as a step entry's `module` names it; a
/// `[[modules]]` entry may not take it.
pub const MODULE: &str = "xmip";

/// Every standard unit's declaration, in the order a person reads them:
/// prepare, validate, promote and demote, transform, process.
#[must_use]
pub fn declarations() -> Vec<&'static Declaration> {
    prepare::DECLARATIONS
        .into_iter()
        .chain([
            contract::validate::DECLARATION,
            promote::DECLARATION,
            demote::DECLARATION,
        ])
        .chain(transform::DECLARATIONS)
        .chain(process::DECLARATIONS)
        .collect()
}

/// The standard module's manifest: it works offline, and so does every
/// unit (ADR-0045, amendment 2026-10-22).
#[must_use]
pub fn manifest() -> ModuleManifest {
    ModuleManifest {
        identity: ModuleIdentity {
            name: MODULE.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        capabilities: Vec::new(),
        entrypoint: ModuleEntrypoint {
            library_path: None,
            executable_path: None,
            symbol: None,
        },
        units: declarations().into_iter().map(entry).collect(),
        offline: true,
    }
}

/// The standard module as the configuration holds a module: what every
/// call naming `module = "xmip"` resolves against, beside the document's
/// own `[[modules]]`.
#[must_use]
pub fn configuration() -> ModuleConfiguration {
    ModuleConfiguration {
        name: MODULE.to_string(),
        start: true,
        manifest: manifest(),
    }
}

/// The document's `modules` with the standard module beside them, and what
/// is wrong with them for it: an entry that takes its name.
#[must_use]
pub fn with_declared(modules: &[ModuleConfiguration]) -> (Vec<ModuleConfiguration>, Vec<String>) {
    let shadowing: Vec<String> = modules
        .iter()
        .filter(|module| module.name == MODULE)
        .map(|_| {
            format!(
                "a [[modules]] entry is called '{MODULE}', the name of the standard module \
                 built in; call it something else"
            )
        })
        .collect();
    let mut every: Vec<ModuleConfiguration> = modules
        .iter()
        .filter(|module| module.name != MODULE)
        .cloned()
        .collect();
    every.push(configuration());
    (every, shadowing)
}

/// What a call to the standard unit `unit` giving `given` is refused for
/// beyond its declaration, one sentence each: a Script runtime Xmip does not
/// host, JavaScript by name (the owner, 2026-10-22: *Keeping JavaScript out
/// of it from the begining is one way*). The rule is
/// `transform::declaration::script_refusal`, the one a starting node calls.
#[must_use]
pub fn refusals(unit: &str, given: &[(String, Given)]) -> Vec<String> {
    if unit != transform::declaration::SCRIPT {
        return Vec::new();
    }
    given
        .iter()
        .filter(|(name, _)| name == "runtime")
        .filter_map(|(_, value)| match value {
            Given::Text(runtime) => transform::declaration::script_refusal(runtime),
            _ => None,
        })
        .collect()
}

/// One declaration as a manifest lists it.
fn entry(declaration: &Declaration) -> ModuleUnit {
    ModuleUnit {
        name: declaration.name.to_string(),
        capability: UnitCapability::ALL
            .into_iter()
            .find(|capability| capability.word() == declaration.capability)
            .unwrap_or(UnitCapability::Transform),
        stages: declaration
            .stages
            .iter()
            .map(|stage| stage.name().to_string())
            .collect(),
        offline: true,
        settings: declaration.settings.settings.iter().map(setting).collect(),
    }
}

/// One declared setting as a manifest writes it.
fn setting(declared: &Setting) -> UnitSetting {
    let (minimum, maximum, choices) = match declared.kind {
        Kind::Integer { minimum, maximum } => (Some(minimum), Some(maximum), Vec::new()),
        Kind::Choice { choices } => (
            None,
            None,
            choices.iter().map(|choice| (*choice).to_string()).collect(),
        ),
        _ => (None, None, Vec::new()),
    };
    let (presence, default) = match declared.presence {
        Presence::Required => ("required", None),
        Presence::Optional => ("optional", None),
        Presence::Default(fixed) => ("default", Some(fixed_default(fixed))),
    };
    UnitSetting {
        name: declared.name.to_string(),
        kind: declared.kind.word().to_string(),
        minimum,
        maximum,
        choices,
        presence: presence.to_string(),
        default,
        meaning: declared.meaning.to_string(),
    }
}

fn fixed_default(fixed: Fixed) -> UnitDefault {
    match fixed.given() {
        Given::Integer(value) => UnitDefault::Integer(value),
        Given::Boolean(value) => UnitDefault::Boolean(value),
        Given::Text(text) => UnitDefault::Text(text),
        Given::Other(word) => UnitDefault::Text(word.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_module_declares_every_unit_once_soundly_and_offline() {
        let manifest = manifest();
        assert_eq!(manifest.identity.name, "xmip");
        assert!(manifest.offline);
        let mut names: Vec<&str> = manifest
            .units
            .iter()
            .map(|unit| unit.name.as_str())
            .collect();
        assert_eq!(names.len(), 13);
        for unit in &manifest.units {
            assert!(unit.offline, "{}", unit.name);
        }
        for declaration in declarations() {
            assert!(
                declaration.problems().is_empty(),
                "{:?}",
                declaration.problems()
            );
        }
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 13, "a unit is declared twice");
        assert!(
            configuration().problems().is_empty(),
            "{:?}",
            configuration().problems()
        );
    }

    #[test]
    fn an_entry_taking_the_standard_name_is_refused_and_the_standard_is_always_there() {
        let (every, shadowing) = with_declared(&[configuration()]);
        assert_eq!(every.len(), 1);
        assert!(
            shadowing[0].contains("standard module built in"),
            "{shadowing:?}"
        );
        let (every, shadowing) = with_declared(&[]);
        assert_eq!(every[0].name, "xmip");
        assert!(shadowing.is_empty());
    }

    #[test]
    fn a_script_in_javascript_is_refused_by_name() {
        let given = |runtime: &str| vec![("runtime".to_string(), Given::Text(runtime.to_string()))];
        assert!(
            refusals("Script", &given("javascript"))[0].contains("JavaScript is not a runtime")
        );
        assert!(refusals("Script", &given("powershell")).is_empty());
        assert!(refusals("Convert", &given("javascript")).is_empty());
    }
}
