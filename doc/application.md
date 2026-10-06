# The Xmip Application

An **Xmip Application** is an integration as a developer designs it: its
routes, and later its transforms and Work Processes, drawn once and deployed to
whichever nodes run it (ADR-0064, `doc/terminology.md`). It is TOML in the
repository (ADR-0031), diffed, reviewed and merged like code. The Route view
of the VS Code designer is a view of this text; a developer may edit either,
and the other follows.

Developers author it among the sections of the cluster's one `xmip.toml`,
as an `[[xmip_applications]]` entry, and nowhere else (ADR-0064 and
ADR-0031, amendments 2026-10-03;
[`cluster-configuration.md`](cluster-configuration.md)); the slice gives it
to the nodes that bind it, and `parse_application` is its one reading.

The Application holds the design and nothing of an environment. Addresses,
credentials and which node takes what are the node's, in its **binding**
(`node-configuration.md`, *Binding an Xmip Application*).

## The shape

From `module/platform/configure/src/application.rs` (`XmipApplication`) and
`port.rs`:

- **`name`**: the developer's name for the integration, what a binding
  names it by. Required.
- **`[[xmip_applications.receive_ports]]`**: the Receive Ports, by `name`.
  A Receive Port keeps Message creation and Publication
  (`runtime-model.md` section 6).
- **`[[xmip_applications.receive_locations]]`**: where Messages enter: a
  `name`; the `receive_port` it belongs to, which the Application declares;
  its `interaction`, `composite`, `data-transfer` or `batch-load`, which
  decides when the sender is acknowledged; and its `depth`, `transfer`,
  `light` or `context`, which decides how far the receive gates read
  (`runtime-model.md` section 7). A Location without a Port, or without
  either statement, is refused (ADR-0031, amendment 2026-10-01).
- **`[[xmip_applications.work_processes]]`**: the Work Processes a
  Subscription may route to, by `name`. The flow is the process designer's,
  after the Work Process vocabulary is settled (ADR-0064 clause 1).
- **`[[xmip_applications.send_ports]]`**: where Messages leave: a `name`,
  and its policy (`runtime-model.md` section 10, the same amendment):
  `send_locations`, its Send Locations by name, tried in order;
  `retry = { attempts, backoff }` on the active Location, the backoff a
  duration such as `5s`; `failover`, `next` or `none`; `execution_style`,
  `sequential`, `parallel` or `concurrent`; `order_key`, what a sequence is
  ordered by; and `on_failure`, `block` or `skip`. A Sequential Send Port
  without `on_failure` is refused: section 3 allows no silent default.
- **`[[xmip_applications.send_port_groups]]`**: Send Ports a Subscription
  reaches together: `name` and `send_ports`, the names of the Send Ports it
  holds.
- **`[[xmip_applications.subscriptions]]`**: what each published Message is
  offered to. Each is `route`'s own Subscription, not a copy of it: an `id`;
  a `destination`, one of `{ work-process = "…" }`, `{ send-port = "…" }` or
  `{ send-group = "…" }`; and a `filter`, one line of Xmip's expression
  language (`xmip-core-path`'s `expression`, ADR-0066): names with the
  prefix of the route technology that reads them (ADR-0046), text in single
  quotes, integers and `true`/`false`; `=`, `<>`, `<`, `<=`, `>`, `>=`,
  `[not] like`, `[not] in (…)`, `exists`; `and`, `or`, `not`; `||`,
  arithmetic and `coalesce`. `"true"` is everything published. The filter is
  compiled as the Application is read: one that does not parse, or compares
  a value with the wrong kind, refuses the Application, and so the node
  that binds it as it starts.

Every list may be left out and reads as empty. A key the Application does
not define, and a word a key does not have, are refused, so a misspelled
list is a problem the developer sees rather than a part of the design that
silently went missing.

## An example

```toml
# The orders integration: orders arrive by file drop and by HTTP, large ones
# go to approval, and every other order reaches billing and the ledger.
[[xmip_applications]]
name = "Orders"

[[xmip_applications.receive_ports]]
name = "Orders"

[[xmip_applications.receive_locations]]
name = "OrdersDrop"
receive_port = "Orders"
interaction = "batch-load"
depth = "context"

[[xmip_applications.receive_locations]]
name = "OrdersApi"
receive_port = "Orders"
interaction = "data-transfer"
depth = "context"

[[xmip_applications.work_processes]]
name = "Approval"

[[xmip_applications.send_ports]]
name = "Billing"
retry = { attempts = 3, backoff = "5s" }
execution_style = "sequential"
order_key = "party"
on_failure = "block"

[[xmip_applications.send_ports]]
name = "Ledger"

[[xmip_applications.send_port_groups]]
name = "Books"
send_ports = ["Billing", "Ledger"]

[[xmip_applications.subscriptions]]
id = "large-orders"
destination = { work-process = "Approval" }
filter = "MessageType = 'Order' and Amount > 1000"

[[xmip_applications.subscriptions]]
id = "orders"
destination = { send-group = "Books" }
filter = "MessageType = 'Order' and not Amount > 1000"
```

A value that is not there is *unknown*, never a silent false: an order with
no `Amount` matches neither Subscription, and each says why it declined
(`nothing promoted Amount`).

## What makes a design unsound

`XmipApplication::problems` says it, one sentence each, and
`xmip_validate_v1` reports it for every surface — the VS Code extension,
`xmip validate`, `Test-XmipNodeConfiguration` — with the binding that binds
it, at startup phase 3:

- an Application, or any part of it, without a name;
- a name declared twice in one list, or a Subscription id used twice;
- a Receive Location without its `receive_port`, naming one the Application
  does not declare, or without its `interaction` or `depth`;
- a Sequential Send Port without `on_failure`, a retry of no attempts or
  a backoff that is not a duration, an empty `order_key`, and a Send
  Location named empty or twice;
- a Send Port Group holding a Send Port the Application does not declare;
- a Subscription routing to a Work Process, Send Port or Send Port Group
  the Application does not declare.

## What the runtime holds of it

The execution tree a node builds as it starts holds what its bindings give
it: the Receive Ports its bound Receive Locations are at, each Location
with its interaction and depth, and the Send Ports it takes with their
policy (`configure::Bound`, the runtime's `ExecutionTree`; ADR-0031,
amendment 2026-10-01).

## The designer's view of it

The cluster designer asks the runtime's library, which forwards to this
crate (`xmip_operate.h` section 10, `views.rs` and `view_edit.rs` over the
cluster's file). Its Receive Port view lists each Application's
`[[xmip_applications.receive_ports]]` and adds one there; its Receive
Location and Send Port views show the keys above, each edited in place. The
Route view:

- **The routes** (`routes.rs`): the Application as a graph — Receive
  Locations, Subscriptions, Work Processes, Send Port Groups and Send Ports,
  each with its place along the route; every Receive Location *publishes*
  to every Subscription, a Subscription *routes* to its destination, and a
  Send Port Group *gathers* its Send Ports.
- **A filter** (`filter.rs`): its line and its rows — a view of the
  compiled expression, never a second grammar. A row is a property (the
  left side, in the language, usually a name), an operator (the language's
  own: `=`, `<>`, `<`, `<=`, `>`, `>=`, `like`, `not like`, `in`,
  `not in`, `exists`) and a value with the kind it is read as (`text`,
  `integer`, `boolean`, or `expression` for a value written in the
  language, such as another name or the list an `in` holds), gathered in
  `and` and `or` groups with `not` around any part. The designer writes a
  filter only when it edits one, in the canonical form, which reads back
  to the same rows and writes again byte for byte; a line written by hand
  is never reformatted by reading it.
- **An edit** (`edit.rs`): declare a Receive Location, a Work Process or a
  Send Port; add a Subscription routing to a target; set a Subscription's
  filter; connect a Subscription to another target. The edit changes the
  lines it touches and leaves every comment, order and layout elsewhere as
  it was.
