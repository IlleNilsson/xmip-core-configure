# xmip-core-configure

Reads and validates declared Xmip configuration and turns it into the typed service configuration consumed by the runtime.

It owns configuration interpretation, not runtime execution or durable state.

`XmipConfigurationDocument` is the one model of a node's configuration in
the estate, read by `parse_toml`. The runtime builds its execution tree from
it, and every surface validates through the runtime's `xmip_validate_v1`,
which reads it here; the desktop editor is a view over the same document and
keeps no model of its own. A location's `start` and `transport` have no
default: a document without them is refused. A Process's `required_modules`,
`xmip_subprocesses` and `extensions` default to empty (ADR-0031, amendment
2026-09-24). The shape is in
[`doc/node-configuration.md`](doc/node-configuration.md).

Until 2026-09-24 a second tree, `XmipServiceConfiguration`, restated the
document under other names and the runtime read that; it is gone (open
problem 25, row b).
