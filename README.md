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
through the runtime's library (`xmip_operate.h` section 10). An operator who
opens a Subscription is shown its entry as the file says it,
`subscription_entry`, read from the Application's text with its layout and
comments (ADR-0013, amendment 2026-09-30). The shapes are
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
the declarations it carries. A Receive Location's `accept` is the closed set
of mechanisms it authenticates (ADR-0019 clause 1, `Accept`); it is refused
on a Send Location.

`[store]` says where the node keeps its runtime store — its place, its key
store and where that keeps its keys — and `[service] data` its data
directory (`store.rs`, ADR-0018 amendment 2026-09-30). Every key has a
default: `<data>/persistence-rocksdb`, the platform's key store at
`<data>/key`, `<data>` being `../data` from the configuration file. The
engine is RocksDB and no node's choice (ADR-0015 and ADR-0018, amendments
2026-10-01); `[store] engine` is refused as an unknown key.

`[storage] nodes` lists the Storage nodes the node reaches Xmip Storage
at, `host:port` each, tried round robin (`storage.rs`,
`deployment-model.md` section 7), and `[storage.database]` names the
database server a Storage node is in front of: two connections, the
runtime and the administration database, the secret its password is kept
under and the authority the server's certificate reaches (`database.rs`;
what IT sets up for it is `deploy/database/<server>/README.md`). An address
without its port, a connection that is not `<server>://<login>@<host>[:<port>]/<database>`,
a Storage node named twice and one database named for both are refused in
words.
