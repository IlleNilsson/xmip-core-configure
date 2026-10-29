# xmip-core-configure

Reads and validates declared Xmip configuration and turns it into the typed service configuration consumed by the runtime.

It owns configuration interpretation, not runtime execution or durable state.

`XmipConfigurationDocument` is the one model of a node's configuration in
the estate, read by `parse_toml`. `XmipApplication` is the Xmip Application
a developer designs (ADR-0064), a section of the cluster's one `xmip.toml`,
`[[xmip_applications]]`, read by `parse_application` and checked by
`problems`; its Subscriptions are `route`'s own, never modeled again. A
node's configuration binds the sections it runs in `[[applications]]`, and
`bind` joins the two into what one node runs: the Locations its bindings
give it, the Receive Ports they are at, the Send Ports they are, and every
Subscription of every bound Application.

An Application's Receive Ports are `[[receive_ports]]`, and each Receive
Location names its `receive_port`, refused without one, and states its
`interaction` (`composite`, `data-transfer`, `batch-load`) and its `depth`
(`transfer`, `light`, `context`); a Send Port states its policy —
`send_locations` tried in order, `retry = { attempts, backoff }`,
`failover` (`next`, `none`) and the `correlation` its Journeys pass it by —
and a Receive Location, a Receive Port and a Work Process may name a
`correlation` too, declared once for the cluster in `[[correlations]]`
(`correlation.rs`, its keys and `on_failure`, `block` or `skip`; `port.rs`;
ADR-0031, amendment 2026-10-01; ADR-0024 and ADR-0066, amendments
2026-10-22). A Work Process names its compiled flow by `module` and `unit`. Each is a problem
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
over the same document and keeps no model of its own. A Location is
entered by its address alone: `address` has no default, so a document
without it is refused, and the scheme it is entered with chooses the
transport — a path or `file://`, `ftp://` or `ftps://`, `sftp://user@host`,
`http://` or `https://` — read once by `ConfiguredLocation::scheme`
(ADR-0064, amendment 2026-10-28). A Receive Location, Send Port or Send
Location starts unless its `start` says `false`, when it is known and not
started, which every surface shows as `NOT_STARTED`, "not started: the
configuration says false". A
Work Process's `required_modules` and `extensions` default to empty (ADR-0031, amendment 2026-09-24).

A Location's `settings` and `contract_settings` tables are held to the
declaration their technology makes of them (`location_problems`, ADR-0064
amendment 2026-09-26): unknown, other-side, wrong-kind and missing required
settings are refused, each naming the technology and the setting. Which
settings there are is never this crate's — the shape is `xmip-core`'s
`settings`, each declaration its technology's, and the caller hands over
the declarations it carries. A Receive Location's `accept` is the closed set
it authenticates (ADR-0019 clause 1, `Accept`): the mechanisms, and the
Parties by their `[[parties]]` names, a name none declares a problem the
binding reports (`party::accepted_problems`); it is refused on a Send
Location. Each Party is written with its `id`, a UUIDv7 minted once with
it (`PartyConfiguration::new`, `party::mint`; the designer's `AddEntry`
mints it for a Party it adds), and each identity with its `mechanism`, its
`purpose` — `receive` or `send` — and its `value` (`PartyIdentity`; the
owner, 2026-10-19); a Party without an id, or with one that is not a
UUIDv7 or is another's, is a problem the binding reports in words
(`party::problems`). A verified identity resolves to the Party holding its
value under the mechanism that verified it, for the purpose asked
(`party::holding`; ADR-0019, amendment 2026-10-20).

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
own sections, artifact by artifact: `views` answers one view per kind
(`kinds.rs`) — Cluster, Node, Party, Module, Receive Port, Receive
Location, Send Port, Send Location, Send Port Group, Prepare, Promote,
Demote, Route, Transformation, Work Process — with each entry's values
(`field`), what may be set on it (`offered`, the runtime handing in its
`[tuning]` declaration and the role words through `Views::offering`), an
Application's routes, and every compiled unit the file's modules declare,
and `view_edit` makes the designer's edits in place — `add-element` adding
one element to an array of unnamed tables, a Party's identity — writing `start = true` on
each Receive Location, Send Port and Send Location it creates, and refusing
one that would leave a node or an Application that read unable to.
Prepare, Promote, Demote and Transformation are calls to compiled units
(ADR-0066, amendment 2026-10-22; `step.rs`, `modules.rs`): a Module's
manifest declares its units (`abi::ModuleUnit`: a name, a capability, the
stages it serves in `xmip-core-node`'s stage words, `offline = true`, and
its settings), `[[preparation_steps]]`, `[[promotions]]`, `[[demotions]]`,
`[[transformations]]` and `[[debatches]]` — in a node's document and in an
Application — call one each with settings, and a Port or Location attaches
calls by name, `prepare`, `transform`, `promote` and `debatch` on receive
and `demote` on send (ADR-0066, amendment 2026-10-27).
`step::problems` refuses, in words, a module or unit not declared offline,
a module or unit nobody declares, a unit of another capability, settings
its unit's declaration refuses, an attachment naming nothing, and a unit
attached at a stage it does not serve; `step::unrun` is what a node refuses
to start, since running a unit is decided, not built.
`parse_toml` refuses a cluster's file, and `document_kind` tells it apart,
so `xmip_validate_v1` validates it node by node. `[tuning]` holds every
outward and hardware assumption a node runs by; its keys, bounds and
defaults are the runtime's declaration (`tuning::TUNING`). With the
`test-support` feature, `fixture::test_cluster` reads the estate's test
cluster, `test/xmip.toml` or the file `XMIP_TEST_CLUSTER` names: the one
place a test takes a cluster's or a node's name from.

A node's top-level `roles` is the role words its cluster's
`[nodes.<name>] roles` declares, as the slice gives them; their parse is
`node::NodeRole`'s, and only a node declaring `storage` keeps Xmip Storage
itself (ADR-0056, amendment 2026-10-20). A cluster's shared top declares
no roles: the editor's keys (`keys.rs`) offer `roles` under a node's own
table alone. `[storage] nodes` lists the Storage nodes a node without the
role reaches Xmip Storage at, `host:port` each, tried round robin
(`storage.rs`, `deployment-model.md` section 7). `[runtime]`, `[administration]` and
`[audit]` say what each of Xmip Storage's three data domains is kept on, a
table each, its `storage` — `rocksdb`, `sqlite`, `postgresql`, `mssql` or
`mariadb` — and its `connection`, a path or the server's own connection
string; a table left out is the embedded Storage node's own under the data
directory. `[storage.database]` names the secret a database server's
password is kept under and the authority its certificate reaches
(`database.rs`; what IT sets up is `deploy/database/<server>/README.md`).
An address without its port, a storage or a connection Xmip Storage does
not read, a Storage node named twice and one database named for two
domains are refused in words.
