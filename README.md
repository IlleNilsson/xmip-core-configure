# xmip-core-configure

Reads and validates declared Xmip configuration and turns it into the typed service configuration consumed by the runtime.

It owns configuration interpretation, not runtime execution or durable state.

`XmipConfigurationDocument` is the one model of a node's configuration in
the estate, read by `parse_toml`. `XmipApplication` is the Xmip Application
a developer designs (ADR-0064), a section of the cluster's one `xmip.toml`,
`[[xmip_applications]]`, read by `parse_application` and checked by
`problems`; its Subscriptions are `route`'s own, never modelled again. A
node's configuration binds the sections it runs in `[[applications]]`, and
`bind` joins the two into what one node runs: the Locations its bindings
give it, the Receive Ports they are at, the Send Ports they are, and every
Subscription of every bound Application.

An Application's Receive Ports are `[[receive_ports]]`, and each Receive
Location names its `receive_port`, refused without one, and states its
`interaction` (`composite`, `data-transfer`, `batch-load`) and its `depth`
(`transfer`, `light`, `context`); a Send Port states its policy —
`send_locations` tried in order, `retry = { attempts, backoff }`,
`failover` (`next`, `none`), `execution_style`, `order_key` and
`on_failure` (`block`, `skip`) — and a Sequential one without `on_failure`
is refused (`port.rs`; ADR-0031, amendment 2026-10-01). Each is a problem
of the Application, refused at startup phase 3, and held in the runtime's
execution tree. The designer's Route view
of an Application — its routes as a graph (`routes`), a filter's rows and
groups as a view of its compiled expression (`filter`; the filter itself is
one line of Xmip's expression language, `xmip-core-path`'s `expression`,
compiled as the Application is read, ADR-0066), and the edits it makes to
the text in place
(`edit`) — is here too, reached by the VS Code extension's language server
through the runtime's library (`xmip_operate.h` section 10). An operator who
opens a Subscription is shown its entry as the file says it,
`subscription_entry`, read from the configuration's text with its layout and
comments (ADR-0013, amendment 2026-09-30). The shapes are
in [`doc/node-configuration.md`](doc/node-configuration.md) and
[`doc/application.md`](doc/application.md).

The runtime builds its execution tree from the node's document and what
`bind` answers, and every surface validates a node's document or a
cluster's file through the runtime's `xmip_validate_v1`, which reads it
here and tells the two apart by the `[nodes]` table (`document_kind`); the
desktop editor is a view
over the same document and keeps no model of its own. A location's `start`
and `transport` have no default: a document without them is refused. A
Work Process's `required_modules` and `extensions` default to empty (ADR-0031, amendment 2026-09-24).

A Location's `settings` and `contract_settings` tables are held to the
declaration their technology makes of them (`location_problems`, ADR-0064
amendment 2026-09-26): unknown, other-side, wrong-kind and missing required
settings are refused, each naming the technology and the setting. Which
settings there are is never this crate's — the shape is `xmip-core`'s
`settings`, each declaration its technology's, and the caller hands over
the declarations it carries. A Receive Location's `accept` is the closed set
of mechanisms it authenticates (ADR-0019 clause 1, `Accept`); it is refused
on a Send Location.

`[store]` names the key store an embedded Storage node's records are
sealed under and where that keeps its keys, and `[service] data` the node's
data directory (`store.rs`, ADR-0018 amendments 2026-09-30 and 2026-10-03).
Every key has a default: the platform's key store at `<data>/key`, `<data>`
being `../data` from the configuration file. The engines are no node's
choice — RocksDB for the runtime database (ADR-0015 and ADR-0018,
amendments 2026-10-01) — and `[store] engine` and `[store] place` are
refused as unknown keys.

A cluster's configuration is one `xmip.toml`: its shared sections once and
each node's under `[nodes.<name>]`, and `slice` is the one slicing of it
into a node's document, the node's value winning (`cluster.rs`; ADR-0031,
amendment 2026-10-03; [`doc/cluster-configuration.md`](doc/cluster-configuration.md)).
Its Xmip Applications are sections of it, `[[xmip_applications]]`
(`section.rs`, ADR-0064 amendment 2026-10-03), and nowhere else: a binding
binds the section of its name, and the slice gives a node the sections its
bindings name. The VS Code designer is a view of the file's
own sections, artifact by artifact: `views` answers one view per kind —
Cluster, Node, Receive Port, Receive Location, Send Port, Send Location,
Send Port Group, Prepare, Promote, Demote, Route, Transformation, Process —
with each entry's values (`field`) and an Application's routes, saying of
a kind the configuration does not define yet that it does not, and
`view_edit` makes the designer's edits in place, refusing one that would
leave a node or an Application that read unable to.
`parse_toml` refuses a cluster's file, and `document_kind` tells it apart,
so `xmip_validate_v1` validates it node by node. `[tuning]` holds every
outward and hardware assumption a node runs by; its keys, bounds and
defaults are the runtime's declaration (`tuning::TUNING`). With the
`test-support` feature, `fixture::test_cluster` reads the estate's test
cluster, `test/xmip.toml` or the file `XMIP_TEST_CLUSTER` names: the one
place a test takes a cluster's or a node's name from.

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
