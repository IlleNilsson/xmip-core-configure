# The node configuration document

Moved here from the estate root on 2026-09-12 (ADR-0020 clause 3: the document lives where its subject lives).


A process is **configuration, not code**. There is no repository and nothing to
compile — you describe a node's work, and the runtime enacts it.

### What a process is made of

From `module/platform/configure/src/lib.rs`:

- **`XmipProcessConfiguration`** — `name`, `start`, `execution_style`,
  `required_modules`, `xmip_subprocesses`, `extensions`.
- **`ExecutionStyle`** — `sequential` (the default), `parallel` or `concurrent`,
  as `doc/architecture/runtime-model.md` section 3, *Execution style*, defines
  them; this document does not redefine them. It is the lever an operator raises
  when a node falls behind (the Playground's `daily` scenario).
- **`ConfiguredLocation`** — a Receive or a Send Location: a `name`, the
  `transport` module that moves it, and the `address` in that transport's own
  terms. A Receive Location runs the identity pipeline (identify → authenticate →
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

Validate the document with `configure::parse_toml`; the desktop editor validates
a half-built document rather than failing at line 1, so you can save work in
progress.

---

