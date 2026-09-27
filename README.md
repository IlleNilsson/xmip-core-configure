# xmip-core-configure

Reads and validates declared Xmip configuration and turns it into the typed service configuration consumed by the runtime.

It owns configuration interpretation, not runtime execution or durable state.

Two documents, each read once here. `XmipConfigurationDocument` is the one
model of a node's configuration in the estate, read by `parse_toml`.
`XmipApplicationDocument` is the Xmip Application a developer designs
(ADR-0064), read by `parse_application` and checked by `problems`; its
Subscriptions are `route`'s own, never modelled again. A node's
configuration binds Applications in `[[applications]]`, and `bind` joins
the two into what one node runs: the Locations its bindings give it and
every Subscription of every bound Application. The routes designer's view
of an Application — its routes as a graph (`routes`), a filter's rows and
groups as a view of its compiled expression (`filter`; the filter itself is
one line of Xmip's expression language, `xmip-core-path`'s `expression`,
compiled as the Application is read, ADR-0066), and the edits it makes to
the text in place
(`edit`) — is here too, reached by the VS Code extension's language server
through the runtime's library (`xmip_operate.h` section 10). The shapes are
in [`doc/node-configuration.md`](doc/node-configuration.md) and
[`doc/application.md`](doc/application.md).

The runtime builds its execution tree from the node's document and what
`bind` answers, and every surface validates either document through the
runtime's `xmip_validate_v1`, which reads it here and tells the two apart by
the `[application]` table (`document_kind`); the desktop editor is a view
over the same document and keeps no model of its own. A location's `start`
and `transport` have no default: a document without them is refused. An
Xmip Process's `required_modules`, `xmip_subprocesses` and `extensions`
default to empty (ADR-0031, amendment 2026-09-24).

A Location's `settings` and `contract_settings` tables are held to the
declaration their technology makes of them (`location_problems`, ADR-0064
amendment 2026-09-26): unknown, other-side, wrong-kind and missing required
settings are refused, each naming the technology and the setting. Which
settings there are is never this crate's — the shape is `xmip-core`'s
`settings`, each declaration its technology's, and the caller hands over
the declarations it carries.

Until 2026-09-24 a second tree, `XmipServiceConfiguration`, restated the
document under other names and the runtime read that; it is gone (open
problem 25, row b).
