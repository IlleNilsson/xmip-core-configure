# The node configuration document

Moved here from the estate root on 2026-09-12 (ADR-0020 clause 3: the document lives where its subject lives).


A process is **configuration, not code**. There is no repository and nothing to
compile — you describe a node's work, and the runtime enacts it.

### What a process is made of

From `module/platform/configure/src/lib.rs`:

- **`XmipProcessConfiguration`** — `name`, `start`, `execution_style`,
  `required_modules`, `xmip_subprocesses`, `extensions`. The last three
  default to empty when omitted, as the document's own lists do, and so do a
  Subprocess's `required_modules` and `extensions` (ADR-0031, amendment
  2026-09-24); `name` and `start` have no default.
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

A process is the flow **Receive Location → process (and subprocesses) → Send
Location**, referencing transport and contract *modules* by name.

### Two ways to author it

1. **Xmip Operations (the desktop app)** — the intended path. Navigate the tree,
   add a process, set its execution style, add Receive and Send Locations that
   point at your transport and contract modules. The operator observes, reasons,
   then tweaks or adds a node.
2. **The configuration TOML directly** — the same document Xmip Operations reads
   and writes. Minimal shape:
   ```toml
   [service]
   name = "…"; cluster_name = "…"; node_name = "…"

   [[xmip_processes]]
   name = "invoices"
   start = true
   execution_style = "sequential"      # or "parallel" / "concurrent"
   required_modules = ["xmip-core-contract-csv"]

   [[receive_locations]]
   name = "drop"
   start = true
   transport = "xmip-core-transport-file"
   address = "/var/xmip/in/invoices"

   [[send_locations]]
   name = "forward"
   start = true
   transport = "xmip-core-transport-<name>"
   address = "…"
   ```

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
name = "xmip-alpha"
cluster_name = "orders"
node_name = "alpha"

[[applications]]
name = "Orders"                       # [application] name in its document
document = "orders.application.toml"  # relative to this file

[[applications.receive_locations]]
name = "OrdersDrop"                   # declared by the Application
node = "alpha"                           # the node that takes it
start = true
transport = "xmip-core-transport-file"
address = "/var/xmip/in/orders"

[[applications.receive_locations]]
name = "OrdersApi"
node = "alpha"
start = true
transport = "xmip-core-transport-http"
address = "https://xmip.example/orders"
credentials = "orders-api"            # a secret's name, never the secret

[[applications.send_ports]]
name = "Billing"
node = "beta"
start = true
transport = "xmip-core-transport-http"
address = "https://billing.example/orders"

[[applications.send_ports]]
name = "Ledger"
node = "beta"
start = true
transport = "xmip-core-transport-file"
address = "/var/xmip/out/ledger"
```

A bound Location has a Location's own shape (`ConfiguredLocation`: `name`,
`start`, `transport`, `address` and the optional `credentials`, `contract`,
`settings`, `contract_settings` and `accept`) and one more key, `node`. The binding is the same on every node of the cluster; each node
takes what names it. From `module/platform/configure/src/binding.rs`:

- **`binding_problems`** checks what a binding says on its own: a name, a
  document, and every bound Location's name, node and transport, none bound
  twice. `xmip_validate_v1` reports these for the text an editor holds.
- **`bind`** joins the bindings to the Applications they name and takes
  what this node runs: the bound Receive Locations and Send Ports whose
  `node` is this node's `node_name`, the Send Ports as the Send Locations
  they leave by, every Subscription of every bound Application and every
  Send Port Group, which a Subscription routed to a group reaches. It
  refuses an Application the node was not given, a bound Application's own
  problems, a Location the Application does not declare, and a Subscription
  id or Location name two of them would put on the node twice.

The runtime builds its execution tree from what `bind` answers
(`build_execution_tree`), and `xmip_start_v1` reads each bound document from
the path its binding names, relative to the configuration file. A text
validated alone has no files beside it, so its bindings are checked as far
as they say on their own; the join is checked when the node starts. Each
Subscription's filter — one line of Xmip's expression language, such as
`filter = "MessageType = 'Order' and not Amount > 1000"` — is compiled as its
Application is read, so a node binding an Application whose filter does not
compile is refused as it starts, not at its first Message (ADR-0066).

`configure::parse_toml` reads the document, and it is the only reading of it:
the runtime's `xmip_validate_v1` validates through it, and every surface — the
desktop editor, the language server, `xmip validate`,
`Test-XmipNodeConfiguration` — asks the runtime rather than judging for
itself. The desktop editor saves a half-built document so you can keep work in
progress, and shows the runtime's verdict on it.

---

