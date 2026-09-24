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
  leaves out `start` or `transport` is refused, not completed. A Receive Location runs the identity pipeline (identify → authenticate →
  authorize, ADR-0019); a Send Location presents identity (ADR-0033).

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

`configure::parse_toml` reads the document, and it is the only reading of it:
the runtime's `xmip_validate_v1` validates through it, and every surface — the
desktop editor, the language server, `xmip validate`,
`Test-XmipNodeConfiguration` — asks the runtime rather than judging for
itself. The desktop editor saves a half-built document so you can keep work in
progress, and shows the runtime's verdict on it.

---

