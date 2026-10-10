# The node configuration document

Moved here from the estate root on 2026-09-12 (ADR-0020 clause 3: the document lives where its subject lives).

A node's document is written once for the whole cluster, in its
`xmip.toml`, and sliced to each node as it is deployed: every section below
is written there at the top for every node, or under `[nodes.<name>]` for
one ([`cluster-configuration.md`](cluster-configuration.md); ADR-0031,
amendment 2026-10-03).


A Work Process passes through four things, kept apart here because only
two of them are built:

1. **Declaration** — `[[work_processes]]` in this document, or an Xmip
   Application's Work Process bound by it: its name, whether it starts, its
   execution style and the Modules it needs. No repository is created.
2. **Compilation** — what a developer designs in VS Code is compiled at
   design time into a native Module the node loads; the node never
   interprets a design (ADR-0066).
3. **Materialization** — the Xmip Service validates every declaration as the
   node starts and plans it into the execution tree, refusing what names a
   Module or Extension it cannot have, and its Xmip Host Services start it.
4. **Execution** — a Subscription opens a Journey to it, and its Stages run,
   wait and resume with their state in the Ledger (`runtime-model.md`
   section 22).

Declaration and the validation and planning of materialization are
[built, in the assembled service](../../../../doc/architecture/estate-map.md#process-declaration): a node
publishes each Work Process as planned, not started. Compilation, starting
one and execution are [decided, not built](../../../../doc/architecture/estate-map.md#process-execution): a
Journey to a Work Process ends saying no runtime runs it yet.

### What a Work Process is made of

From `module/platform/configure/src/lib.rs`:

- **`WorkProcessConfiguration`** — `name`, `start`, `execution_style`,
  `required_modules`, `extensions`. The last two default to empty when
  omitted, as the document's own lists do (ADR-0031, amendment 2026-09-24);
  `name` and `start` have no default.
- **`ExecutionStyle`** — `sequential` (the default), `parallel` or `concurrent`,
  as `doc/architecture/runtime-model.md` section 3, *Execution style*, defines
  them; this document does not redefine them. It is the lever an operator raises
  when a node falls behind (the Playground's `daily` scenario).
- **`ConfiguredLocation`** — a Receive or a Send Location: a `name`, whether
  it will `start`, the `transport` module that moves it, and the `address` in
  that transport's own terms. None of the four has a default; a document that
  leaves out `start` or `transport` is refused, not completed. `credentials`,
  optional, names the secret the Location presents or checks — a reference,
  never the secret. A Receive Location runs the identity pipeline (identify →
  authenticate → authorize, ADR-0019); a Send Location presents identity
  (ADR-0033). `settings`, `contract` and `contract_settings`, optional, are
  what the Location gives its technologies: *A Location's settings* below.
  `accept`, a Receive Location's alone, is the closed set it authenticates:
  *What a Receive Location accepts* below.
- **`ApplicationBinding`** — `[[applications]]`, an Xmip Application the node
  runs and the environment's side of it; *Binding an Xmip Application* below.

A node's flow is **Receive Location → Work Process → Send Location**, referencing transport and contract *modules* by name.

### Two ways to author it

1. **The Operation Desktop** — the intended path. Navigate the tree, add a
   Work Process, set its execution style, add Receive and Send Locations
   that point at your transport and contract modules. The operator
   observes, reasons, then tweaks or adds a node.
2. **The configuration TOML directly** — the same document the Operation
   Desktop reads and writes. Minimal shape, a node that takes files from
   one folder and puts them in another:
   ```toml
   # a complete node file
   [service]
   name = "xmip-<node>"
   cluster_name = "<cluster>"
   node_name = "<node>"

   [[work_processes]]
   name = "invoices"
   start = true
   execution_style = "sequential"      # or "parallel" / "concurrent"

   [[receive_locations]]
   name = "drop"
   start = true
   transport = "xmip-core-transport-file"
   address = "/var/xmip/in/invoices"

   [[send_locations]]
   name = "forward"
   start = true
   transport = "xmip-core-transport-file"
   address = "/var/xmip/out/invoices"
   ```
   `# a complete node file` marks it as a whole document:
   `test/Example.Test.ps1` at the estate root has the runtime validate every
   block so marked, in this document and every other.

### The key store a node's records are sealed under

A node that lists no Storage node in `[storage]` is its own embedded
Storage node, and Xmip encrypts its databases itself: every record sealed
under a data key a key store wraps (ADR-0063). What a node keeps across a
restart — a paused Subscription's standing in the administration database,
the Journeys it holds in the Ledger (ADR-0013, amendments 2026-09-30 and
2026-10-03) — is kept there. The engines are no node's choice: the runtime
database is always RocksDB (ADR-0015 and ADR-0018, amendments 2026-10-01),
and `engine` and `place` are refused as unknown keys. `[store]` names the
key store and where the audit database is, and every key may be left out
(`src/store.rs`, ADR-0018 amendments 2026-09-30 and 2026-10-03; ADR-0070,
amendment 2026-10-10):

```toml
# a complete node file
[service]
name = "xmip-<node>"
cluster_name = "<cluster>"
node_name = "<node>"
data = "../data"                              # the default

[store]
key_store = "xmip-core-secret-dpapi"          # the platform's, by default
keys = "../data/key"                          # the default
audit = "../data/storage/audit.sqlite"        # the default
```

- **`[service] data`** is the node's data directory, relative to this file;
  absent, `../data`, which in the installed layout is the `data` beside the
  `config` this file is in (ADR-0015 clause 10). The embedded Storage node
  (`<data>/storage`), its keys and the orders an operator leaves for the
  node (`<data>/orders`) are there unless `[store]` says otherwise.
- **`key_store`**: the platform's key store by default —
  `xmip-core-secret-dpapi` on Windows, `xmip-core-secret-keychain` on
  macOS, `xmip-core-secret-file` on every other Unix.
- **`keys`**: where a key store that keeps files keeps them; absent,
  `<data>/key`. The keychain keeps its keys as items and does not read it.
- **`audit`**: the audit database's file, SQLite as the administration
  database's, relative to this file; absent, `<data>/storage/audit.sqlite`,
  beside the runtime database (`<data>/storage/runtime`) and the
  administration database (`<data>/storage/administration.sqlite`). The
  audit database is a data domain of its own, so it may be on other
  storage than the other two (the owner, 2026-10-10: *The audit part might
  be better of in its own database so it can be hosted on a different set
  of nodes, different storage*).

Which key stores a node can use is its program's: `xmip-service` links them
by build feature, as it links transports, and RocksDB and SQLite with them.
A node naming a key store its program was not built with is refused as it
starts, and so is a Storage node whose databases do not open — another
process holding them among the reasons, and a program built without RocksDB
or SQLite.

### What a node runs by: `[tuning]`

Every outward and hardware assumption a node runs by is configured, per
cluster and per node, under `[tuning]` in the cluster's `xmip.toml` and
`[nodes.<name>.tuning]` for one node, the node's winning in its slice
([`cluster-configuration.md`](cluster-configuration.md); ADR-0031,
amendment 2026-10-03). Every key may be left out; its default is the
built-in value:

```toml
[tuning]
tcp_segment = 1460                          # bytes, 536 to 9000
segments = 44                               # TCP segments a chunk holds, 1 to 1024
receive_threads_per_hardware_thread = 2     # 1 to 64
receive_idle = "1m"                         # a receive thread's idle time
send_threads_per_hardware_thread = 2        # 1 to 64
send_idle = "1m"                            # a send thread's idle time
send_lease = "30s"                          # how long a claim on a Journey holds
send_scan = "1s"                            # how often the send queues are read
storage_timeout = "5s"                      # each connect and read to a Storage node
storage_pass_over = "5s"                    # how long one that did not answer waits its turn
```

- **`tcp_segment`** and **`segments`**: a Stream is written to the Ledger
  in chunks of `segments` × `tcp_segment` bytes, 64,240 by default
  (`runtime-model.md` section 3).
- **`receive_threads_per_hardware_thread`** and **`receive_idle`**: a
  Receive Location's pool grows to this many threads per hardware thread
  the machine runs, and a thread with nothing to do ends after its idle
  time.
- **`send_threads_per_hardware_thread`** and **`send_idle`**: the same for
  the node's one Send pool (`runtime-model.md` section 3).
- **`send_lease`** and **`send_scan`**: how long a claim on a Journey holds
  before it lapses unless renewed, and how often the queues of the Send
  Ports this node sends are read for unclaimed Journeys (`runtime-model.md`
  section 10).
- **`storage_timeout`** and **`storage_pass_over`**: what bounds each
  connect to and read from a Storage node, and how long one that did not
  answer is asked only after the rest. Read and checked, and used by
  nothing `xmip-service` runs: it reaches no Storage node of its own
  ([built, not in the assembled service](../../../../doc/architecture/estate-map.md#storage-nodes)).

Durations are a whole number and `ms`, `s`, `m` or `h`, above nothing and
at most an hour. Which keys there are, their kinds, bounds and defaults are
the runtime's one declaration (`xmip-core-runtime`'s `tuning::TUNING`, in
`xmip-core`'s settings shape), and the table is read through it as a
technology reads a Location's settings: an unknown key, a value of the
wrong kind or outside its bounds is refused at startup phase 3, and by
`xmip_validate_v1`, in words.

### Where a node reaches Xmip Storage

Every node calls Xmip Storage — the nodes declaring the Storage role — for
all storage, never a database directly, and finds them from a list of their
addresses, tried round robin (`deployment-model.md` section 7;
`src/storage.rs`):

```toml
[storage]
nodes = ["storage-1.example:7443", "storage-2.example:7443"]
```

- **`nodes`**: each `host:port`, a host name or an address, IPv6 in
  brackets, each once. More than one is the safety: a node carries on
  through the next while one is stopped. An address without its port is
  refused in words, and so is a Storage node named twice.

A Storage node in front of a database server IT runs (option A) names it
(`src/database.rs`):

```toml
[storage.database]
runtime        = "postgresql://xmip_storage@db-1.example:5432/xmip_runtime"
administration = "postgresql://xmip_storage@db-2.example:5432/xmip_administration"
audit          = "postgresql://xmip_storage@db-3.example:5432/xmip_audit"
password       = "xmip-storage-database"       # a secret's name, never the password
trust_anchor   = "../config/database-authority.pem"
```

- **`runtime`**, **`administration`** and **`audit`**: the three databases
  Xmip Storage keeps on every backend, one to each data domain, separate,
  which IT may place on different servers:
  `<server>://<login>@<host>[:<port>]/<database>`, the server `postgresql`
  or `sqlserver` — the same for all three — and the port the server's own
  (5432, 1433) where it is left out.
- **`password`**: the name of the secret the login's password is kept
  under, resolved through the key home (ADR-0063 clause 4); the password is
  never written here.
- **`trust_anchor`**: the authority the server's certificate reaches, PEM,
  relative to this file; absent, the operating system's trust store. Xmip
  always speaks TLS to the database server.

What a site's IT operators install and run for it — the software, the
scripts that make each database and its roles, the settings Xmip
depends on — is `deploy/database/postgresql/README.md` and
`deploy/database/sqlserver/README.md` at the estate root.

### A Location's settings

What a transport takes beyond the address — a topic, a timeout, a queue —
and the contract a Stream is held to, with what that takes, are written on
the Location:

```toml
[[receive_locations]]
name = "orders"
start = true
transport = "xmip-core-transport-kafka"
address = "broker.example:9092"
contract = "xmip-core-contract-json-schema"     # optional

[receive_locations.settings]                     # the transport's
topic = "orders"

[receive_locations.contract_settings]            # the contract's
reference = "schemas/order.json"
```

Which settings there are is never written here, in `configure`, in the
language server or in the desktop editor: **every technology declares its
own** in its own crate — each setting's name, kind (text, integer, boolean,
duration such as `30s`, address, a secret's name, or one of a list),
default or requirement, what it means, and whether a Receive Location, a
Send Location or both read it (ADR-0064, amendment 2026-09-26). A transport
declares them through `transport::Configured`, a contract through
`ContractFactory::settings`, both in the shape `xcore::settings`, and the
technology reads its settings through that same declaration.

`configure::location_problems` holds each table to its technology's
declaration, and `xmip_validate_v1` and `xmip_start_v1` report what it
finds for every Location, the node's own and every bound one: a setting the
technology does not declare, one declared for the other side only, a value
of the wrong kind or outside its range, and a required one left out — each
naming the Location, the technology and the setting — and
`contract_settings` given with no `contract`. A technology is held to its
declaration where the runtime carries it — a node carries each technology
it loads, as it loads it — and a node that starts refuses a Location whose
technology it was not built with; the runtime's library answers which it
carries, and what each declares, through
`xmip_technology_catalogue_v1` (`xmip_operate.h` section 12), which the
VS Code extension's completion and hover read.

The `contract` a Location names is one of five steps every Receive and Send
Port and Location may configure — Prepare, Contract with `validate`,
Transform, and Promote on receive or Demote on send — the Location in its
Party's or endpoint's format, the Port in its one format (ADR-0031,
amendment 2026-10-05; `runtime-model.md` section 20 has the keys). Not
read yet: a node refuses to start a Location that names a `contract`
([decided, not built](../../../../doc/architecture/estate-map.md#arrival-validation)).

### What a Receive Location accepts

A Receive Location declares the **closed set** of mechanisms it
authenticates (ADR-0019 clause 1), each by the name its mechanism declares:

```toml
[receive_locations.accept]
mechanism = ["mutual-tls", "oauth2"]
```

An identity presented by any other mechanism is refused at authentication,
and never tried against the rest. A Receive Location that gives no `accept`
takes nothing — an unconfigured endpoint is closed, not open — and one
receiving from something that presents nothing, a drop folder or a raw
socket, accepts `circumstance`: the circumstance is the transport identity
(ADR-0019 clause 7). A node that starts one refuses a mechanism no
authenticator it was built with verifies. A Send Location presents and
accepts nothing, so `accept` on one is refused. ADR-0019 also writes a
`party` list beside `mechanism`; a node's configuration names no Party yet,
so that key is refused rather than read as nothing.

### Binding an Xmip Application

A node runs the integrations developers design as Xmip Applications
([`application.md`](application.md), ADR-0064). The Application holds the
design and nothing of an environment; the node's configuration **binds**
it: which Application it runs, and the environment's side of what it
declares — each Receive Location's and Send Port's transport, address,
reference to its credentials and the node that takes it, as BizTalk's
bindings do. The same route is never written again per node.

```toml
[service]
name = "xmip-<node>"
cluster_name = "<cluster>"
node_name = "<node>"

[[applications]]
name = "Orders"                       # its [[xmip_applications]] section's name

[[applications.receive_locations]]
name = "OrdersDrop"                   # declared by the Application
node = "<node>"                       # the node that takes it
start = true
transport = "xmip-core-transport-file"
address = "/var/xmip/in/orders"

[[applications.receive_locations]]
name = "OrdersApi"
node = "<node>"
start = true
transport = "xmip-core-transport-http"
address = "https://xmip.example/orders"
credentials = "orders-api"            # a secret's name, never the secret

[[applications.send_ports]]
name = "Billing"
node = "<another>"
start = true
transport = "xmip-core-transport-http"
address = "https://billing.example/orders"

[[applications.send_ports]]
name = "Ledger"
node = "<another>"
start = true
transport = "xmip-core-transport-file"
address = "/var/xmip/out/ledger"
```

A binding binds the Application the configuration holds as its section of
the same name, `[[xmip_applications]]`: the cluster's one `xmip.toml`
writes it, and the slice gives it to the nodes that bind it
([`cluster-configuration.md`](cluster-configuration.md)). `<cluster>`,
`<node>` and `<another>` stand for the names whoever runs the cluster
chooses.

A bound Location has a Location's own shape (`ConfiguredLocation`: `name`,
`start`, `transport`, `address` and the optional `credentials`, `contract`,
`settings`, `contract_settings` and `accept`) and one more key, `node`. The binding is the same on every node of the cluster; each node
takes what names it. From `module/platform/configure/src/binding.rs`:

- **`binding_problems`** checks what a binding says: a name, the section of
  its name held and its design sound, and every bound Location's name, node
  and transport, none bound twice. `xmip_validate_v1` reports these for the
  text an editor holds.
- **`bind`** joins the bindings to the Applications they name and takes
  what this node runs: the bound Receive Locations and Send Ports whose
  `node` is this node's `node_name`, the Send Ports as the Send Locations
  they leave by, the Receive Ports those Receive Locations are at with
  each Location's interaction and depth, the Send Ports as designed with
  their policy, every Subscription of every bound Application and every
  Send Port Group, which a Subscription routed to a group reaches. It
  refuses an Application the node was not given, a bound Application's own
  problems, a Location the Application does not declare, and a Subscription
  id or Location name two of them would put on the node twice.

The runtime builds its execution tree from what `bind` answers
(`build_execution_tree`), and `xmip_start_v1` reads each bound Application
from its section in the configuration. Each
Subscription's filter — one line of Xmip's expression language, such as
`filter = "MessageType = 'Order' and not Amount > 1000"` — is compiled as its
Application is read, so a node binding an Application whose filter does not
compile is refused as it starts, not at its first Message (ADR-0066).

`configure::parse_toml` reads the document, and it is the only reading of it:
the runtime's `xmip_validate_v1` validates through it, and every surface — the
desktop editor, the language server, `xmip-cli validate`,
`Test-XmipNodeConfiguration` — asks the runtime rather than judging for
itself. The desktop editor saves a half-built document so you can keep work in
progress, and shows the runtime's verdict on it.

---

