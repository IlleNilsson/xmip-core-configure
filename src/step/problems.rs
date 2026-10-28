//! What is refused of the calls to compiled units and where they are
//! attached, one sentence each ([`super`], ADR-0066 amendment 2026-10-22).

use abi::UnitCapability;
use node::Stage;
use xcore::settings::Applies;

use super::{Attached, LISTS, Steps, unit_of};
use crate::modules::{ModuleConfiguration, declared, serves};
use crate::{LocationSettings, XmipApplication, XmipConfigurationDocument};

/// Everything wrong with `document`'s calls to compiled units and where it
/// attaches them, one sentence each: its modules' units, its own lists, its
/// Locations' attachments, its Work Processes' flows, and each Xmip
/// Application section it holds, its calls resolved against the node's
/// modules.
#[must_use]
pub fn problems(document: &XmipConfigurationDocument) -> Vec<String> {
    // The standard module is built in: every call resolves against it too.
    let (modules, shadowing) = crate::standard::with_declared(&document.modules);
    let modules = &modules;
    let mut problems: Vec<String> = modules
        .iter()
        .flat_map(ModuleConfiguration::problems)
        .collect();
    problems.extend(shadowing);

    let node = Steps::of_node(document);
    entries_problems("", node, modules, &mut problems);
    let bound = document.applications.iter().flat_map(|binding| {
        let receive = binding.receive_locations.iter();
        let send = binding.send_ports.iter();
        receive
            .map(|bound| (Stage::Receive, &bound.location))
            .chain(send.map(|bound| (Stage::Send, &bound.location)))
    });
    let own = document
        .receive_locations
        .iter()
        .map(|location| (Stage::Receive, location))
        .chain(
            document
                .send_locations
                .iter()
                .map(|location| (Stage::Send, location)),
        );
    for (side, location) in own.chain(bound) {
        let what = format!("the {} '{}'", location_words(side), location.name);
        attached_problems(
            &what,
            side,
            location.attached(),
            node,
            modules,
            &mut problems,
        );
    }
    for process in &document.work_processes {
        let what = format!("the Work Process '{}'", process.name);
        flow_problems(
            &what,
            process.module.as_deref(),
            process.unit.as_deref(),
            &process.settings,
            modules,
            &mut problems,
        );
    }
    for section in &document.xmip_applications {
        if let Ok(application) = section.application() {
            application_problems(&application, modules, &mut problems);
        }
    }
    problems
}

/// An Application's calls resolved against `modules`, and its artifacts'
/// attachments against its own lists.
fn application_problems(
    application: &XmipApplication,
    modules: &[ModuleConfiguration],
    problems: &mut Vec<String>,
) {
    let of = format!(" of '{}'", application.name);
    let steps = Steps::of_application(application);
    entries_problems(&of, steps, modules, problems);
    let ports = application
        .receive_ports
        .iter()
        .map(|port| ("Receive Port", Stage::Receive, &port.name, port.attached()));
    let receives = application.receive_locations.iter().map(|location| {
        let attached = location.attached();
        ("Receive Location", Stage::Receive, &location.name, attached)
    });
    let sends = application
        .send_ports
        .iter()
        .map(|port| ("Send Port", Stage::Send, &port.name, port.attached()));
    for (kind, side, name, attached) in ports.chain(receives).chain(sends) {
        let what = format!("the {kind} '{name}'{of}");
        attached_problems(&what, side, attached, steps, modules, problems);
    }
    for process in &application.work_processes {
        let what = format!("the Work Process '{}'{of}", process.name);
        let module = process.module.as_deref();
        let unit = process.unit.as_deref();
        flow_problems(&what, module, unit, &process.settings, modules, problems);
    }
}

fn location_words(side: Stage) -> &'static str {
    if side == Stage::Send {
        "Send Location"
    } else {
        "Receive Location"
    }
}

/// Each entry of the four lists: a name once, its module and unit
/// declared, the unit of the list's capability, and its settings read.
fn entries_problems(
    of: &str,
    steps: Steps<'_>,
    modules: &[ModuleConfiguration],
    problems: &mut Vec<String>,
) {
    for ((_, capability, person), entries) in LISTS.iter().zip(steps.lists) {
        let mut seen = std::collections::BTreeSet::new();
        for entry in entries {
            let what = format!("the {person} '{}'{of}", entry.name);
            if entry.name.trim().is_empty() {
                problems.push(format!("a {person}{of} requires a name"));
            } else if !seen.insert(entry.name.as_str()) {
                problems.push(format!("{what} is declared twice"));
            }
            call_problems(
                &what,
                &entry.module,
                &entry.unit,
                &entry.settings,
                *capability,
                modules,
                problems,
            );
        }
    }
}

/// A call to `unit` of `module`: both declared, the unit of `capability`,
/// and `settings` as its declaration reads them.
fn call_problems(
    what: &str,
    module: &str,
    unit: &str,
    settings: &LocationSettings,
    capability: UnitCapability,
    modules: &[ModuleConfiguration],
    problems: &mut Vec<String>,
) {
    let Some(declaring) = modules.iter().find(|declared| declared.name == module) else {
        problems.push(format!(
            "{what} names the module '{module}', which no [[modules]] entry declares"
        ));
        return;
    };
    let Some(called) = declaring.unit(unit) else {
        problems.push(format!(
            "{what} names the unit '{unit}', which the module '{module}' does not declare"
        ));
        return;
    };
    if called.capability != capability {
        problems.push(format!(
            "{what} names the unit '{unit}' of '{module}', a {} unit, where a {} unit is \
             called",
            called.capability.word(),
            capability.word()
        ));
    }
    if let Ok(declaration) = declared(module, called)
        && let Err(refused) = declaration.read(Applies::Both, &settings.given())
    {
        for refusal in refused.0 {
            problems.push(format!("{what}: {refusal}"));
        }
    }
    if module == crate::standard::MODULE {
        for refusal in crate::standard::refusals(unit, &settings.given()) {
            problems.push(format!("{what}: {refusal}"));
        }
    }
}

/// A Work Process's flow, where it names one: a `work-process` unit.
fn flow_problems(
    what: &str,
    module: Option<&str>,
    unit: Option<&str>,
    settings: &LocationSettings,
    modules: &[ModuleConfiguration],
    problems: &mut Vec<String>,
) {
    match (module, unit) {
        (None, None) if settings.is_empty() => {}
        (Some(module), Some(unit)) => {
            call_problems(
                what,
                module,
                unit,
                settings,
                UnitCapability::WorkProcess,
                modules,
                problems,
            );
            let called = modules
                .iter()
                .find(|declared| declared.name == module)
                .and_then(|declared| declared.unit(unit));
            if let Some(called) = called.filter(|called| !serves(called, Stage::Process)) {
                problems.push(format!(
                    "{what} names the unit '{unit}' of '{module}', which does not serve the \
                     'process' stage; it serves {}",
                    stage_words(&called.stages)
                ));
            }
        }
        _ => problems.push(format!(
            "{what} names its flow by both module and unit, and gives settings only with them"
        )),
    }
}

/// What an artifact on `side` attaches: each a name its document's list
/// declares, Promote on receive and Demote on send alone, and each unit's
/// stage the artifact's side.
fn attached_problems(
    what: &str,
    side: Stage,
    attached: Attached<'_>,
    steps: Steps<'_>,
    modules: &[ModuleConfiguration],
    problems: &mut Vec<String>,
) {
    if side == Stage::Send && attached.promote.is_some() {
        problems.push(format!(
            "{what} attaches a Promotion, which only a Receive Port or Location runs"
        ));
    }
    if side == Stage::Send && attached.debatch.is_some() {
        problems.push(format!(
            "{what} attaches a Debatch, which only a Receive Port or Location runs, or a \
             Work Process"
        ));
    }
    if side == Stage::Receive && attached.demote.is_some() {
        problems.push(format!(
            "{what} attaches a Demotion, which only a Send Port or Location runs"
        ));
    }
    for (list, name) in attached.named() {
        let person = LISTS
            .iter()
            .find(|(key, _, _)| *key == list)
            .map_or("", |(_, _, person)| *person);
        let Some(entry) = steps.entry(list, name) else {
            problems.push(format!(
                "{what} attaches the {person} '{name}', which [[{list}]] does not declare"
            ));
            continue;
        };
        if let Some(unit) = unit_of(entry, modules)
            && !serves(unit, side)
        {
            problems.push(format!(
                "{what} attaches the {person} '{name}', whose unit '{}' does not serve the \
                 '{}' stage; it serves {}",
                unit.name,
                side.name(),
                stage_words(&unit.stages)
            ));
        }
    }
}

/// A unit's stages, as a sentence names them.
fn stage_words(stages: &[String]) -> String {
    if stages.is_empty() {
        return "none".to_string();
    }
    stages
        .iter()
        .map(|stage| format!("'{stage}'"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::problems;
    use crate::parse_toml;

    /// A node holding a module of three units, the lists calling them and
    /// Locations attaching them; `more` is appended.
    fn node(more: &str) -> Vec<String> {
        let text = format!(
            "[service]\nname = \"n\"\ncluster_name = \"c\"\nnode_name = \"d\"\n\n\
             [[modules]]\nname = \"orders\"\nstart = true\n\
             [modules.manifest]\ncapabilities = []\noffline = true\n\
             [modules.manifest.identity]\nname = \"orders\"\nversion = \"0.1.0\"\n\
             [modules.manifest.entrypoint]\n\
             [[modules.manifest.units]]\nname = \"Codec\"\ncapability = \"prepare\"\n\
             stages = [\"receive\", \"send\"]\noffline = true\n\
             [[modules.manifest.units.settings]]\nname = \"format\"\nkind = \"choice\"\n\
             choices = [\"gzip\", \"zstd\"]\npresence = \"required\"\n\
             [[modules.manifest.units]]\nname = \"Numbers\"\ncapability = \"promote\"\n\
             stages = [\"receive\"]\noffline = true\n\
             [[modules.manifest.units]]\nname = \"Enrich\"\ncapability = \"transform\"\n\
             stages = [\"process\"]\noffline = true\n\n\
             [[preparation_steps]]\nname = \"Unzip\"\nmodule = \"orders\"\nunit = \"Codec\"\n\
             settings = {{ format = \"gzip\" }}\n\n\
             [[promotions]]\nname = \"OrderNumbers\"\nmodule = \"orders\"\nunit = \"Numbers\"\n\n\
             [[transformations]]\nname = \"Enriched\"\nmodule = \"orders\"\nunit = \"Enrich\"\n\n\
             [[receive_locations]]\nname = \"drop\"\ntransport = \"t\"\naddress = \"a\"\n\
             prepare = [\"Unzip\"]\npromote = \"OrderNumbers\"\n{more}"
        );
        let document = parse_toml(&text).expect("reads");
        problems(&document)
    }

    #[test]
    fn calls_and_attachments_that_fit_are_sound() {
        assert_eq!(node(""), [] as [String; 0]);
    }

    #[test]
    fn an_unknown_module_unit_or_capability_is_refused_in_words() {
        let found = node(
            "\n[[demotions]]\nname = \"A\"\nmodule = \"billing\"\nunit = \"Codec\"\n\
             [[demotions]]\nname = \"B\"\nmodule = \"orders\"\nunit = \"Nothing\"\n\
             [[demotions]]\nname = \"C\"\nmodule = \"orders\"\nunit = \"Codec\"\n",
        );
        assert!(
            found[0].contains("no [[modules]] entry declares"),
            "{found:?}"
        );
        assert!(found[1].contains("does not declare"), "{found:?}");
        assert!(
            found[2].contains("a prepare unit, where a demote unit"),
            "{found:?}"
        );
    }

    #[test]
    fn a_calls_settings_are_read_through_its_units_declaration() {
        let wrong = node(
            "\n[[preparation_steps]]\nname = \"Lz\"\nmodule = \"orders\"\nunit = \"Codec\"\n\
             settings = { format = \"lz4\" }\n",
        );
        assert!(
            wrong
                .iter()
                .any(|p| p.contains("'Lz'") && p.contains("format")),
            "{wrong:?}"
        );
        let missing = node(
            "\n[[preparation_steps]]\nname = \"Bare\"\nmodule = \"orders\"\nunit = \"Codec\"\n",
        );
        assert!(missing.iter().any(|p| p.contains("'Bare'")), "{missing:?}");
    }

    #[test]
    fn a_unit_is_attached_only_at_a_stage_it_serves() {
        let found = node(
            "\n[[send_locations]]\nname = \"out\"\ntransport = \"t\"\naddress = \"a\"\n\
             transform = \"Enriched\"\npromote = \"OrderNumbers\"\nprepare = [\"Nope\"]\n",
        );
        assert!(
            found
                .iter()
                .any(|p| p.contains("attaches a Promotion, which only a Receive")),
            "{found:?}"
        );
        assert!(
            found
                .iter()
                .any(|p| p.contains("'Enriched'") && p.contains("does not serve the 'send'")),
            "{found:?}"
        );
        assert!(
            found
                .iter()
                .any(|p| p.contains("'Nope', which [[preparation_steps]]")),
            "{found:?}"
        );
    }

    /// A splitting unit, its entry and `attach` on a Location of `side`.
    fn debatching(side: &str, attach: &str) -> Vec<String> {
        node(&format!(
            "\n[[modules.manifest.units]]\nname = \"Lines\"\ncapability = \"debatch\"\n\
             stages = [\"receive\", \"process\"]\noffline = true\n\n\
             [[debatches]]\nname = \"PerLine\"\nmodule = \"orders\"\nunit = \"Lines\"\n\
             [[debatches]]\nname = \"NotOne\"\nmodule = \"orders\"\nunit = \"Enrich\"\n\n\
             [[{side}_locations]]\nname = \"split\"\ntransport = \"t\"\naddress = \"a\"\n\
             debatch = \"{attach}\"\n"
        ))
    }

    #[test]
    fn a_debatch_is_a_debatch_unit_attached_on_receive_alone() {
        let found = debatching("receive", "PerLine");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].contains("'NotOne'") && found[0].contains("a transform unit, where a debatch"),
            "{found:?}"
        );
        let sent = debatching("send", "PerLine");
        assert!(
            sent.iter()
                .any(|p| p.contains("attaches a Debatch, which only a Receive")),
            "{sent:?}"
        );
        let unknown = debatching("receive", "Nothing");
        assert!(
            unknown
                .iter()
                .any(|p| p.contains("'Nothing', which [[debatches]] does not declare")),
            "{unknown:?}"
        );
    }

    #[test]
    fn a_work_process_calls_a_flow_that_serves_the_process_stage() {
        let found = node(
            "\n[[work_processes]]\nname = \"w\"\nstart = true\nmodule = \"orders\"\n\
             unit = \"Codec\"\n[[work_processes]]\nname = \"half\"\nstart = true\n\
             module = \"orders\"\n",
        );
        assert!(
            found
                .iter()
                .any(|p| p.contains("a prepare unit, where a work-process"))
        );
        assert!(
            found
                .iter()
                .any(|p| p.contains("does not serve the 'process' stage"))
        );
        assert!(
            found
                .iter()
                .any(|p| p.contains("'half' names its flow by both"))
        );
    }
}
