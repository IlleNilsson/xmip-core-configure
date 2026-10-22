//! `[[work_processes]]`: an Xmip Work Process a node declares, how it runs
//! its work and what it needs — among it the compiled unit that is its flow
//! (ADR-0066, amendment 2026-10-22).

use abi::ExtensionManifest;
use serde::{Deserialize, Serialize};

use crate::LocationSettings;

/// How a Work Process runs its work, once claimed — runtime-model.md's
/// "execution style". `Sequential` is the safe default (one at a time, in order
/// per key); `Parallel` and `Concurrent` trade ordering for throughput, and are
/// the lever an operator raises when a node falls behind (the Playground's
/// `daily` scenario). Serialized kebab-case on the wire.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionStyle {
    /// One at a time, in order per key.
    #[default]
    Sequential,
    /// Many at once, no ordering guarantee.
    Parallel,
    /// Many in flight, interleaved, no ordering guarantee.
    Concurrent,
}

/// `[[work_processes]]`: one Work Process and what it needs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkProcessConfiguration {
    pub name: String,
    pub start: bool,
    /// Defaults to `Sequential` when the document omits it, so an existing
    /// configuration reads unchanged.
    #[serde(default)]
    pub execution_style: ExecutionStyle,
    /// The two lists below default to empty, as the document's own lists
    /// do: a Work Process that needs no module and no Extension says
    /// nothing about them (ADR-0031, amendment 2026-09-24).
    #[serde(default)]
    pub required_modules: Vec<String>,
    #[serde(default)]
    pub extensions: Vec<ExtensionManifest>,
    /// Its flow, a call to a compiled unit: the `[[modules]]` entry holding
    /// it, the unit's name as that module's manifest declares it, a
    /// `work-process` unit of the `process` stage, and what the call gives
    /// ([`crate::step`]). Absent, the Work Process names no flow yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "LocationSettings::is_empty")]
    pub settings: LocationSettings,
}
