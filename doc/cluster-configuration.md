# The cluster's configuration, `xmip.toml`

A cluster's configuration is one file, `xmip.toml` (ADR-0031, amendment
2026-10-03: *There is one xmip.toml file per cluster. When deployed the
sections regarding a node will be sliced to that node*). What the whole
cluster shares is written once, at the top; what concerns one node is under
`[nodes.<name>]`, in the same sections a node's own document has
([`node-configuration.md`](node-configuration.md)):

```toml
[service]
name = "xmip"
cluster_name = "<cluster>"

[tuning]                                   # every node's, unless its own says
segments = 44

[[receive_locations]]
name = "drop"
start = true
transport = "xmip-core-transport-file"
address = "/var/xmip/in"

[nodes.<node>.service]
name = "xmip-<node>"

[nodes.<node>.tuning]                      # this node's, winning
receive_threads_per_hardware_thread = 4

[[nodes.<node>.receive_locations]]         # merged with "drop" by its name
name = "drop"
address = "/srv/xmip/in"

[nodes.<another>.runtime]
storage    = "postgresql"
connection = "host=db-1.example dbname=xmip_runtime user=xmip_storage"

[nodes.<another>.audit]
storage    = "postgresql"
connection = "host=db-2.example dbname=xmip_audit user=xmip_storage"

[nodes.<another>.storage.database]
password = "xmip-storage-database"
```

`<cluster>`, `<node>` and `<another>` stand for the names whoever runs the
cluster chooses.

## Xmip Applications, among its sections

The Xmip Applications developers design are sections of the same file
(ADR-0064, amendment 2026-10-03: *Developers, Operators deal with the one
xmip.toml file*), and nowhere else. Each is one `[[xmip_applications]]`
entry, its `name` and its lists beneath it
([`application.md`](application.md)). A binding binds the section of its
name:

```toml
[[xmip_applications]]
name = "Orders"

[[xmip_applications.receive_ports]]
name = "Orders"

[[xmip_applications.receive_locations]]
name = "OrdersIn"
receive_port = "Orders"
interaction = "data-transfer"
depth = "light"

[[xmip_applications.send_ports]]
name = "Billing"

[[xmip_applications.subscriptions]]
id = "billing"
destination = { send-port = "Billing" }
filter = "MessageType = 'Order'"

[[nodes.<node>.applications]]             # the binding
name = "Orders"

[[nodes.<node>.applications.receive_locations]]
name = "OrdersIn"
node = "<node>"
start = true
transport = "xmip-core-transport-file"
address = "/var/xmip/in/orders"
```

`ApplicationSection` (`src/section.rs`) reads a section as the Application
it is, through `parse_application`, the one reading of an Application.
`binding_problems` refuses a binding where the configuration holds no
section of its name, and reports a bound section's design problems; the
runtime reads each bound section as the node starts (`xmip-core-runtime`,
`start.rs`).

## The slice

`configure::slice(cluster, node)` (`src/cluster.rs`) is the one slicing: it
writes the node `node` its configuration document, which
`configure::parse_toml` reads as it reads any node's.

- **The shared sections and the node's own, the node's value winning where
  both say one.** Tables merge key by key. An array of tables whose every
  entry has a `name` — `[[receive_locations]]`, `[[send_locations]]`,
  `[[work_processes]]`, `[[applications]]` — merges entry by entry on the
  name: an entry the node names that the cluster has is merged into it, one
  the cluster does not have is added. Any other value the node gives
  replaces the cluster's.
- **`[service] node_name` is the node's key under `[nodes]`.** The slice
  writes it. A cluster's shared `[service]` naming one is refused, and so is
  a node's own naming another than its key.
- **A node the cluster does not declare is refused**, naming the
  `[nodes.<name>]` tables it does.
- **An Xmip Application section goes to the nodes that bind it**: a
  node's slice keeps the `[[xmip_applications]]` entries its bindings —
  the cluster's shared `[[applications]]` and its own — name, and no
  other.
- `configure::cluster::nodes` names the nodes a cluster's file declares, and
  `configure::slices` slices every one.

**Only this file is edited.** A node's `xmip-node.toml` is its slice, written
by desired state and never edited; one changed by hand breaks the
configuration rules and is written over at the next deployment (ADR-0031,
amendment 2026-10-05).

A node never reads the cluster's file itself: `parse_toml` refuses a
document holding `[nodes]`, saying it is a cluster's to slice.

## Who slices

- **Desired state**, as it deploys each node (`deployment-model.md`
  section 8): the Ansible role `xmip_node` places the cluster's `xmip.toml`
  and writes the node `xmip-service --configuration <xmip.toml> --node <name>
  --slice`'s output as its `xmip-node.toml`. It does not take the file apart
  itself; the estate root's `cargo test --test deploy` renders it and reads
  what it writes.
- **The Operation Desktop**, when its Configure page saves the cluster's
  file (ADR-0031, amendment 2026-10-05: *the cluster TOML file is sliced
  into node TOML files and shipped to each node on save*): through the
  runtime's `xmip_cluster_slices_v1` (`xmip_operate.h` section 10), node by
  node, each slice written as `<node>/xmip-node.toml` where the desktop
  keeps them. The node the desktop starts reads its slice there; another
  node's is written and not shipped, since no Xmip path yet puts a file on
  another node — desired state, above, slices it on the node.
- **Validation**: `xmip_validate_v1` tells a cluster's file apart by its
  `[nodes]` (`configure::document_kind`) and validates it node by node,
  each problem opening with its node's location,
  `xmip:///<cluster>/node/<name>` (ADR-0027 clause 4). Every surface that
  validates a text through it — the desktop editor, the language server,
  `xmip-cli validate`, `Test-XmipNodeConfiguration` — is answered for a
  cluster's file the same way.

## Saving is not delivering

Five things happen between an edit and a node running by it, and they are
different acts with different states (ADR-0031, amendments 2026-10-01 and
2026-10-05; open problem 31):

| Act | What it is | State |
| --- | --- | --- |
| **Saving** | the cluster's `xmip.toml` written, validated by the runtime, and audited | [built, in the assembled service](../../../../doc/architecture/estate-map.md#configuration-saving) |
| **Slicing** | the file taken apart into each node's `xmip-node.toml`, by the desktop or by desired state | [built, in the assembled service](../../../../doc/architecture/estate-map.md#configuration-saving) |
| **Delivery** | a slice put on its node, over Xmip's own transport | [decided, not built](../../../../doc/architecture/estate-map.md#configuration-delivery): desired state places the cluster's file and slices it on the node; the desktop delivers only to the node it starts itself |
| **Acceptance** | the node validating what it was given and answering accepted or refused | [decided, not built](../../../../doc/architecture/estate-map.md#configuration-delivery) |
| **Activation** | the node running by it: a changed slice takes effect when what reads it starts again, never mid-flight | a whole node's restart is [built, in the assembled service](../../../../doc/architecture/estate-map.md#configuration-saving); restarting only the threads or Host Services that changed is [decided, not built](../../../../doc/architecture/estate-map.md#configuration-delivery) |

A save that says *OK* has saved and sliced; it has not delivered, been
accepted or been activated anywhere but on the desktop's own node.

## The designer's views of it

The VS Code designer is a view of the file's own sections, artifact by
artifact (ADR-0064, amendment 2026-10-03), and so is the Operation
Desktop's Configure page; what both show and do is this crate's, reached
through the runtime's `xmip_operate.h` section 10:

- **`views::Views::of`** (`src/views.rs`) answers one view per kind, in
  this order: Cluster (`[service]`, `[tuning]`, `[runtime]`,
  `[administration]`, `[audit]`, `[storage]`, `[store]`),
  Node (each `[nodes.<name>]`), Receive Port, Receive Location, Send Port,
  Send Location, Send Port Group, Prepare, Promote, Demote, Route,
  Transformation, Process. A list kind finds its entries wherever the file
  holds them — the cluster's shared sections, a node's own, a binding, an
  Application section — each with the path an edit names it by, whose it
  is, and its values as TOML writes them (`src/field.rs`), and says where a
  new one may be added. A Route entry is an Application section with its
  routes as a graph (`routes.rs`), or the reason it does not read; the
  bindings are Route entries too. Receive Port, Prepare, Promote, Demote
  and Transformation are answered as not defined, with a sentence saying
  so: `[[receive_ports]]` is recorded in `runtime-model.md` and not read
  here yet, and the others have no section. Work Process lists the
  `[[work_processes]]` entries and says that a Work Process's flow is
  not defined yet.
- **`view_edit::apply`** (`src/view_edit.rs`) makes one edit in place —
  set or remove a value, add or remove an entry of a list, declare a node,
  or one of an Application's own edits (`edit.rs`) on its section — keeping
  every comment and layout it does not touch. It refuses an edit that
  would leave a node that sliced and read unable to, or an Application
  section that read unable to, in the reader's words.

## The test cluster

The estate's tests take every cluster's and node's name from one file, the
estate's `test/xmip.toml`, or the file the variable `XMIP_TEST_CLUSTER`
names: `configure::fixture::test_cluster()` (feature `test-support`) reads
it through the slicing above and answers its name and its nodes, each found
by the roles it declares (`with_role`) or its place (`node`), never by a
written name.
