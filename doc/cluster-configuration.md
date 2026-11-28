# The cluster's configuration, `cluster.xmip`

A cluster's configuration is one file, `cluster.xmip` (ADR-0031, amendment
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
address = "/var/xmip/in"

[nodes.<node>.service]
name = "xmip-<node>"

[nodes.<node>.tuning]                      # this node's, winning
receive_threads_per_hardware_thread = 4
local_timeout = "500ms"                    # a call to its local endpoint waits this long

[nodes.<node>.operate]                     # its local endpoint starts without this
operators = ['<DOMAIN>\<operator>']        # at xmip-<node>; local names another

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

Each node declares its roles in its own table, `roles`, in
`node::NodeRole`'s words by comma or plus (ADR-0056, amendment 2026-10-01),
never in the shared top; the slice gives them to the node's own document as
its top-level `roles`. Only a node declaring `storage` keeps Xmip Storage
itself; every other one names the Storage nodes in `[storage] nodes`, and
a node with neither is refused as it starts (amendment 2026-10-20):

```toml
[nodes.<node>]
roles = "executing, storage"
```

Beside its roles, which are Xmip's, a node may declare its **Node
capabilities** in its own table, `capabilities`: words naming what its
environment gives it — a firewall opening, software installed, a system it
reaches — each lowercase letters, digits and hyphens, none twice, refused in
words otherwise (`node::Capability::node_capabilities`; ADR-0056, amendment
2026-11-24). Xmip reads no meaning from them; work that `requires` them is
placed only on a node declaring each (below). Never in the shared top, as
roles are not:

```toml
[nodes.<node>]
roles = "processing"
capabilities = ["approval-module", "firewall-party-x"]
```

The nodes are the file's in the order it first writes each, never their
names' order (`cluster::nodes`, `roles`, `capabilities`; open problem 104):
a TOML table keeps no order of the file's, so the order is read from the
document itself (`document::keys`).

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
address = "/var/xmip/in/orders"
```

### One machine, a file in and a file out

The smallest cluster that carries a Message: one node keeping Xmip Storage
itself, a file Receive Location accepting what is dropped in its folder by
the folder's implied identity, `circumstance`, a Subscription taking every
such file, and a file Send Location writing each in its directory under the
name it was dropped under (open problem 33, repaired 2026-10-20). A build
from the site `personal-computer` carries everything it names.

```toml
[service]
name = "xmip"
cluster_name = "<cluster>"

[[xmip_applications]]
name = "FileCopy"

[[xmip_applications.receive_ports]]
name = "In"

[[xmip_applications.receive_locations]]
name = "Drop"
receive_port = "In"
depth = "light"

[[xmip_applications.send_ports]]
name = "Out"

[[xmip_applications.subscriptions]]
id = "every-dropped-file"
destination = { send-port = "Out" }
filter = "xmip.transport.mechanism = 'circumstance'"

[nodes.<node>]
roles = "executing, storage"

[[nodes.<node>.applications]]
name = "FileCopy"

[[nodes.<node>.applications.receive_locations]]
name = "Drop"
node = "<node>"
address = "/var/xmip/in"
[nodes.<node>.applications.receive_locations.accept]
mechanism = ["circumstance"]
transport = "claimed"

[[nodes.<node>.applications.send_ports]]
name = "Out"
node = "<node>"
address = "/var/xmip/out"
```

### Placed by what each node declares

An environment that writes no bindings by hand — the System Tests' run, and
`xmip-service --slice --bind --nodes` on machines of their own — has them
written by `configure::placed` (`src/placed.rs`), the one placement for
both, so the two never place differently. Each piece of work requires its
stage served (ADR-0056, amendment 2026-11-13): a Receive Location the
receive stage, a Send Port the send stage, a Work Process the process
stage, and a node satisfies it where one of the roles its `[nodes.<name>]
roles` declares serves that stage, `executing` serving every stage. Work
naming Node capabilities in its `requires` requires a node declaring each
of them as well (ADR-0056, amendment 2026-11-24):

```toml
[[xmip_applications.work_processes]]
name = "Approval"
module = "approval"
unit = "Approval"
requires = ["approval-module"]       # only a node declaring it runs it
```

Each Receive Location and Send Port goes to one node, round robin in the
file's order over the nodes present that satisfy it (`xcluster::place`),
whatever order the nodes are handed over in, one rotation for each
requirement, so work requiring more never moves the turn of work requiring
less; each Work Process to every node present that satisfies it
(`xcluster::every`), which share its turns through Xmip Storage. Where
`requires` is absent, a piece of work is placed as before, on any node
serving its stage. Nothing names a node: which node takes the work is
placement's, and a Subscription filters on what a Message holds, never on
a node. A Work Process is bound by its name and its node alone:

```toml
[[applications.work_processes]]
name = "Purchase order confirmation"
node = "<node>"
```

Work no node present satisfies is refused, never left unplaced, naming
the requirement, every node considered and the work, and, where the work
requires Node capabilities, what each node declares:

```text
REFUSED: no node satisfies work requiring serving Send; considered <node> (receiving; offline), <another> (storage; offline). Not placed: the Send Port 'To A' of 'Orders'.
REFUSED: no node satisfies work requiring serving Work Process, approval-module; considered <node> (executing; offline; declares none). Not placed: the Work Process 'Approval' of 'Orders'.
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
- **`[service] short_name` is declared once, for every node.** The file's
  shared `[service]` says the cluster's short name, what every operator
  surface shows for it (`node-configuration.md`; ADR-0028, amendment
  2026-11-28), and the slice gives it to every node, signed with the rest.
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
- **A file may leave the cluster's name to its environment.**
  `configure::slice_named(cluster, node, name)` slices as `slice` does, the
  cluster called `name` where the file's `[service]` names none, its service
  `xmip-<node>` where nothing names it, and refuses a file naming another
  cluster, naming both; `configure::cluster::cluster_name` says which a file
  names. A cluster's file that says nothing of where it runs — no cluster
  name, no machine's address, no absolute path — is the same file on one
  computer and on machines of their own (ADR-0028, amendment 2026-10-21):
  the System Tests' run names the cluster, and `xmip-service --slice
  --cluster` and `--install --cluster` on a machine.

**Only this file is edited.** A node's `node.xmip` is its slice, written
by desired state and never edited (ADR-0031, amendment 2026-10-05). Every
slice is signed by the cluster's authority as it is written, for its node
and under a strictly increasing serial, in a `[signature]` table
(`configure::signed`), and a node runs only one that verifies against the
authority's certificate in the `authority` folder beside it, that
certificate being the one its pin holds, for the node it pinned and above
the highest serial it has seen. Any other is refused and audited, and so is
one that verifies and that the node cannot start; the node runs its active
configuration, the last slice that ran, instead (ADR-0031, amendments
2026-11-06, 2026-11-07 and 2026-11-12).

A node never reads the cluster's file itself: `parse_toml` refuses a
document holding `[nodes]`, saying it is a cluster's to slice.

## Who slices

- **Desired state**, as it deploys each node (`deployment-model.md`
  section 8): the environment templates — Microsoft DSC v3's
  `deploy/dsc/msdsc/environment` and Ansible's role `xmip_node` — run
  `xmip-service --configuration <cluster.xmip> --node <name> --cluster
  <cluster> --authority <folder> --slice` on the machine the environment is
  deployed from, signed by the environment's authority kept there, and ship
  the node its output as `node.xmip` with the authority's certificate
  beside it; a node never slices (ADR-0031, amendment 2026-11-06). Neither
  takes the file apart itself; the estate root's
  `cargo test --test deploy` slices the System Tests' file, which the example
  environment runs, for every node it declares and reads each slice as
  `xmip-service` does.
- **The Operation Desktop**, when its Configure page saves the cluster's
  file (ADR-0031, amendment 2026-10-05: *the cluster TOML file is sliced
  into node TOML files and shipped to each node on save*): through the
  runtime's `xmip_cluster_signed_slice_v1` (`xmip_operate.h` section 10),
  node by node, each slice signed by the cluster's authority kept in this
  account's own Xmip folder on the machine, never beside the cluster's file
  (ADR-0031, amendment 2026-11-25),
  and written as `<node>/node.xmip` where the desktop keeps them, with the
  authority's certificate beside it in `<node>/authority/authority.pem`. The node the desktop starts reads its slice there; another
  node's is written and not shipped, since no Xmip path yet puts a file on
  another node — desired state, above, slices it on the node.
- **Validation**: `xmip_validate_v1` tells a cluster's file apart by its
  `[nodes]` (`configure::document_kind`) and validates it node by node,
  each problem opening with its node's location,
  `xmip:///<cluster>/node/<name>` (ADR-0027 clause 4). Every surface that
  validates a text through it — the desktop editor, the language server,
  `xmip validate`, `Test-XmipNodeConfiguration` — is answered for a
  cluster's file the same way.

## Saving is not delivering

Five things happen between an edit and a node running by it, and they are
different acts with different states (ADR-0031, amendments 2026-10-01 and
2026-10-05; open problem 31):

| Act | What it is | State |
| --- | --- | --- |
| **Saving** | `cluster.xmip` written, validated by the runtime, and audited | [built, in the assembled service](../../../../doc/architecture/estate-map.md#configuration-saving) |
| **Slicing** | the file taken apart into each node's `node.xmip`, by the desktop or by desired state | [built, in the assembled service](../../../../doc/architecture/estate-map.md#configuration-saving) |
| **Delivery** | a slice put on its node, over Xmip's own transport | [decided, not built](../../../../doc/architecture/estate-map.md#configuration-delivery): desired state places the cluster's file and slices it on the node; the desktop delivers only to the node it starts itself |
| **Acceptance** | the node validating what it was given and answering accepted or refused | the signature checked against the node's pin as it starts, a slice above the serial seen pinned as seen before anything reads it, and a refused file audited, is built (`runtime::signed`); the node's answer is its record at `<node>/configuration` and `xmip-service --activation`; the answer back to the save is [decided, not built](../../../../doc/architecture/estate-map.md#configuration-delivery) |
| **Activation** | the node running by it: a changed slice takes effect when what reads it starts again, never mid-flight | a whole node's restart is [built, in the assembled service](../../../../doc/architecture/estate-map.md#configuration-saving): every startup phase and the program's own steps run over the slice, serving nothing, and only then is it kept as the active configuration and served; refused at any step, the node runs its active configuration and tries the slice again at its next start (`runtime::running::activation`; ADR-0031, amendment 2026-11-12); restarting only the threads or Host Services that changed is [decided, not built](../../../../doc/architecture/estate-map.md#configuration-delivery) |

A save that says *OK* has saved and sliced; it has not delivered, been
accepted or been activated anywhere but on the desktop's own node.

## The designer's views of it

The VS Code designer is a view of the file's own sections, artifact by
artifact (ADR-0064, amendment 2026-10-03), and so is the Operation
Desktop's Configure page; what both show and do is xmip-core-operate's
designer, reading the file through this crate and reached through its
library's `xmip_operate.h` section 10 (ADR-0064, amendment 2026-11-16);
the paths below are that repository's:

- **`views::Views::of`** (`src/design/views.rs`, the kinds in `src/design/kinds.rs`)
  answers one view per kind, in this order: Cluster (`[service]`,
  `[tuning]`, `[runtime]`, `[administration]`, `[audit]`, `[storage]`,
  `[store]`, `[operate]`, `[provision]`), Node (each `[nodes.<name>]`),
  Party, Module, Receive Port, Receive Location, Send Port, Send Location,
  Send Port Group, Prepare, Promote, Demote, Route, Transformation, Work
  Process. A list kind finds its entries wherever the file holds them — the
  cluster's shared sections, a node's own, a binding, an Application
  section — each with the path an edit names it by, whose it is, and its
  values as TOML writes them (`src/design/field.rs`), and says where a new one may
  be added. A Route entry is an Application section with its routes as a
  graph (`src/design/routes.rs`), or the reason it does not read; the bindings are
  Route entries too. Prepare, Promote, Demote, Transformation and Debatch
  list the calls to compiled units, `[[preparation_steps]]`,
  `[[promotions]]`, `[[demotions]]`, `[[transformations]]` and
  `[[debatches]]`, and Module the `[[modules]]`
  whose manifests declare the units (ADR-0066, amendment 2026-10-22;
  [`node-configuration.md`](node-configuration.md)); the answer also says
  every unit the file's modules declare, its stages and its settings.
- **What may be set** (`src/design/offered.rs`; ADR-0064, amendment 2026-10-22):
  each entry carries `offered`, every key its table may hold and does not
  write, its kind, whether it is required and the words or names it may
  take; an array of unnamed tables, a Party's `identities`, is offered as
  the keys of one element, added by `add-element`. The runtime hands in
  what it owns, the `[tuning]` declaration and the role words
  (`Views::offering`).
- **`view_edit::apply`** (`src/design/view_edit.rs`) makes one edit in place —
  set or remove a value, add or remove an entry of a list, declare a node,
  or one of an Application's own edits (`src/design/edit.rs`) on its section — keeping
  every comment and layout it does not touch. An entry it adds whose table
  reads `start` and goes without it — a Receive Location, Send Port or Send
  Location — it writes `start = true`, unless the edit gives `start`
  itself: only `true` starts one (the owner, 2026-10-13). It refuses an edit that
  would leave a node that sliced and read unable to, or an Application
  section that read unable to, in the reader's words.
- **What an editor needs** (ADR-0064, amendment 2026-10-13), in the same
  views answer: `written` (`src/design/written.rs`), every table by its section
  with the bytes of its header, its name, the text it covers and each key
  and value; `references` (`src/design/references.rs`), each name that stands for
  an entry declared elsewhere — a Receive Location's `receive_port`, a Send
  Port Group's `send_ports`, a Subscription's `destination`, a binding's
  `name`, each bound Location's `name` and `node`, a node's own
  `[service] node_name` — with what it names, where that is declared and
  every name it may take; and `keys` (`src/design/keys.rs`), every key each table
  of the file may hold, its kind, the words it reads and whether it is
  required, read from this crate's own types by listening to their
  `Deserialize` (`src/design/shape.rs`, `src/design/probe.rs`), never listed a second
  time. Edits that follow them (`src/design/reference_edit.rs`): `rename` renames
  an entry or a node and every name standing for it; `remove-entry` is
  refused while anything names the entry, saying which, unless
  `references`, which removes the entries those names belong to (where the
  name is what names that entry or a key it cannot be without), an array's
  element, or the key; `refer` points a name at another entry;
  `rename-key` renames a key, its value and place kept. A Receive
  Location's `accept.party` names Parties of `[[parties]]`, a call's
  `module` and `unit` its module and unit, and a Port's or Location's
  `prepare`, `transform`, `promote`, `demote` and `debatch` the calls of
  its document,
  offering only those whose unit serves its stage
  (`src/design/references/calls.rs`).

## The test cluster

The estate's tests take every cluster's and node's name from one file, the
estate's `test/cluster.xmip`, or the file the variable `XMIP_TEST_CLUSTER`
names: `configure::fixture::test_cluster()` (feature `test-support`) reads
it through the slicing above and answers its name and its nodes, each found
by the roles it declares (`with_role`) or its place (`node`), never by a
written name.
