# The Xmip Application document

An **Xmip Application** is an integration as a developer designs it: its
routes, and later its transforms and Xmip Processes, drawn once and deployed to
whichever nodes run it (ADR-0064, `doc/terminology.md`). It is a TOML
document in the repository (ADR-0031), diffed, reviewed and merged like
code. The routes designer in VS Code is a view of this text; a developer
may edit either, and the other follows.

The Application holds the design and nothing of an environment. Addresses,
credentials and which node takes what are the node's, in its **binding**
(`node-configuration.md`, *Binding an Xmip Application*).

## The shape

From `module/platform/configure/src/application.rs`
(`XmipApplicationDocument`):

- **`[application]`**: `name`, the developer's name for the integration.
  Required.
- **`[[receive_locations]]`**: where Messages enter, by `name`.
- **`[[xmip_processes]]`**: the Xmip Processes a Subscription may route
  to, by `name`. The flow is the process designer's, after the Xmip
  Process vocabulary is settled (ADR-0064 clause 1).
- **`[[send_ports]]`**: where Messages leave, by `name`.
- **`[[send_port_groups]]`**: Send Ports a Subscription reaches together:
  `name` and `send_ports`, the names of the Send Ports it holds.
- **`[[subscriptions]]`**: what each published Message is offered to. Each
  is `route`'s own Subscription, not a copy of it: an `id`; a
  `destination`, one of `{ process = "…" }`, `{ send-port = "…" }` or
  `{ send-group = "…" }`; and a `filter`, one line of Xmip's expression
  language (`xmip-core-path`'s `expression`, ADR-0066): names with the
  prefix of the route technology that reads them (ADR-0046), text in single
  quotes, integers and `true`/`false`; `=`, `<>`, `<`, `<=`, `>`, `>=`,
  `[not] like`, `[not] in (…)`, `exists`; `and`, `or`, `not`; `||`,
  arithmetic and `coalesce`. `"true"` is everything published. The filter is
  compiled as the document is read: one that does not parse, or compares a
  value with the wrong kind, refuses the document, and so the node that
  binds it as it starts.

Every list may be left out and reads as empty. A key the document does not
define is refused, so a misspelled list is a problem the developer sees
rather than a part of the design that silently went missing.

## An example

```toml
# The orders integration: orders arrive by file drop and by HTTP, large ones
# go to approval, and every other order reaches billing and the ledger.
[application]
name = "Orders"

[[receive_locations]]
name = "OrdersDrop"

[[receive_locations]]
name = "OrdersApi"

[[xmip_processes]]
name = "Approval"

[[send_ports]]
name = "Billing"

[[send_ports]]
name = "Ledger"

[[send_port_groups]]
name = "Books"
send_ports = ["Billing", "Ledger"]

[[subscriptions]]
id = "large-orders"
destination = { process = "Approval" }
filter = "MessageType = 'Order' and Amount > 1000"

[[subscriptions]]
id = "orders"
destination = { send-group = "Books" }
filter = "MessageType = 'Order' and not Amount > 1000"
```

A value that is not there is *unknown*, never a silent false: an order with
no `Amount` matches neither Subscription, and each says why it declined
(`nothing promoted Amount`).

## What makes a design unsound

`XmipApplicationDocument::problems` says it, one sentence each, and
`xmip_validate_v1` reports it for every surface — the VS Code extension,
`xmip validate`, `Test-XmipNodeConfiguration` — because the runtime reads
either document and tells them apart by the `[application]` table, never by
the file's name:

- an Application, or any part of it, without a name;
- a name declared twice in one list, or a Subscription id used twice;
- a Send Port Group holding a Send Port the Application does not declare;
- a Subscription routing to an Xmip Process, Send Port or Send Port Group
  the Application does not declare.

## The designer's view of it

The routes designer asks the runtime's library, which forwards to this
crate (`xmip_operate.h` section 10):

- **The routes** (`routes.rs`): the Application as a graph — Receive
  Locations, Subscriptions, Xmip Processes, Send Port Groups and Send Ports,
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
- **An edit** (`edit.rs`): declare a Receive Location, an Xmip Process or a
  Send Port; add a Subscription routing to a target; set a Subscription's
  filter; connect a Subscription to another target. The edit changes the
  lines it touches and leaves every comment, order and layout elsewhere as
  it was.
