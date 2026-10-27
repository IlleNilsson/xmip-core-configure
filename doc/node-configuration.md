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
publishes each Work Process as planned, not started. Compilation at design
time and execution are [built, not in the assembled service](../../../../doc/architecture/estate-map.md#process-execution)
(ADR-0066, amendment 2026-10-22): a Work Process naming its unit by `module`
and `unit` is opened from that module's library as the node starts, and the
node's Work Process step takes a turn of its Instance for each Journey bound
for it.

### What a Work Process is made of

From `module/platform/configure/src/lib.rs`:

- **`WorkProcessConfiguration`** — `name`, `start`, `required_modules`,
  `extensions`, and `module`, `unit` and `settings`, the compiled Work Process
  unit that is its flow (*Compiled units* below; ADR-0066, amendment
  2026-10-22). The lists default to empty when
  omitted, as the document's own lists do (ADR-0031, amendment 2026-09-24);
  `name` and `start` have no default. How many of its Journeys go at once is
  the Correlation its Application names it by (`runtime-model.md`
  section 3, *Sequences*).
- **`Correlation`** — `[[correlations]]`: `name`, `keys` and `on_failure`,
  declared once for the cluster (`correlation.rs`).
- **`ConfiguredLocation`** — a Receive or a Send Location: a `name`, whether
  it will `start`, the `transport` module that moves it, and the `address` in
  that transport's own terms. `name`, `transport` and `address` have no
  default; a document that leaves out `transport` is refused, not completed.
  A Location starts only when its `start` says `true`; nothing or `false`
  means not started — the node knows it, opens nothing for it, and every
  surface shows it as "not started: the configuration does not say true"
  (`configure::NOT_STARTED`; the owner, 2026-10-13). A bound Send Port is
  held to the same rule, and the designer writes `start = true` on every
  Receive Location, Send Port and Send Location it creates. `credentials`,
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
   roles = "executing, storage"        # one machine keeps Xmip Storage itself

   [service]
   name = "xmip-<node>"
   cluster_name = "<cluster>"
   node_name = "<node>"

   [[work_processes]]
   name = "invoices"
   start = true
   module = "invoices"                  # the [[modules]] entry holding its unit
   unit = "Invoices"                    # the unit, as its manifest names it

   [[receive_locations]]
   name = "drop"
   start = true
   transport = "xmip-core-transport-file"
   address = "/var/xmip/in/invoices"
   [receive_locations.accept]
   mechanism = ["circumstance"]        # a drop folder's implied identity

   [[send_locations]]
   name = "forward"
   start = true
   transport = "xmip-core-transport-file"
   address = "/var/xmip/out/invoices"  # a directory: each file under its received name
   ```
   `# a complete node file` marks it as a whole document:
   `test/Example.Test.ps1` at the estate root has the runtime validate every
   block so marked, in this document and every other.

### The key store a node's records are sealed under

A node declaring the `storage` role that lists no Storage node in
`[storage]` is its own embedded Storage node; only a node with the Storage
role stores, and one declaring neither the role nor `[storage] nodes` is
refused as it starts, in words naming both (ADR-0056, amendment
2026-10-20). Its roles are the top-level `roles`, the role words its
cluster's `[nodes.<name>] roles` declares, which the slice writes there
(`src/lib.rs`, [`cluster-configuration.md`](cluster-configuration.md)).
An embedded Storage node's databases Xmip encrypts itself: every record sealed
under a data key a key store wraps (ADR-0063). What a node keeps across a
restart — a paused Subscription's standing in the administration database,
the Journeys it holds in the Ledger (ADR-0013, amendments 2026-09-30 and
2026-10-03) — is kept there. The engines are no node's choice: the runtime
database is always RocksDB (ADR-0015 and ADR-0018, amendments 2026-10-01),
and `engine` and `place` are refused as unknown keys. `[store]` names the
key store, and every key may be left out (`src/store.rs`, ADR-0018
amendments 2026-09-30 and 2026-10-03):

```toml
# a complete node file
roles = "storage"                             # it keeps Xmip Storage itself

[service]
name = "xmip-<node>"
cluster_name = "<cluster>"
node_name = "<node>"
data = "../data"                              # the default

[store]
key_store = "xmip-core-secret-dpapi"          # the platform's, by default
keys = "../data/key"                          # the default
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

Which key stores a node can use is its program's: `xmip-service` links them
by build feature, as it links transports, and RocksDB and SQLite with them.
A node naming a key store its program was not built with is refused as it
starts, and so is a Storage node whose databases do not open — another
process holding them among the reasons, and a program built without RocksDB
or SQLite.

### Where it is operated from: `[operate]`

`[operate]` is the node's operate listener (ADR-0067; ADR-0014, amendments
2026-10-12 and 2026-10-13): where a surface reads the node's publication,
follows it and acts on it, over gRPC. `local` is its endpoint on this
machine — `xmip --local`, `-Local`, a GUI whose `Surface` is `local` — a
named pipe on Windows and a Unix domain socket elsewhere, opened only by the
node's account, the machine's administrators (the superuser on Unix) and each
account or group `admit` names, every caller the account the operating system
says its process runs as — decided by the owner, 2026-10-19: *local callers
are proven by the operating system*; a tcp connection over loopback is
trusted for nothing and presents a certificate like any other (open problem
35). A name in `admit` the system does not know is refused as the node
starts. On Unix, where `admit` names anyone, the socket is `0660` and
belongs to `group`, the node's own group where it is left out: the file
system lets only that group's members connect, and the node judges each
against `admit` after (the owner, 2026-10-21); a group the system does not
know, or one the node's account is no member of, is refused as the node
starts. **It always starts** — decided by
the owner, 2026-10-13: *the local endpoint always starts, named `xmip-<node>`
unless the configuration names it* — at `\\.\pipe\<name>` on Windows,
`/run/xmip/<name>.sock` on Linux and `/var/run/xmip/<name>.sock` on macOS,
the folder the systemd unit or a launchd daemon makes for the node's
account; a name may change, the folder may not, and a name that would leave
it is refused. `listen` is its address for another machine — `xmip
--remote`, `-Remote`, a GUI whose `Surface` is `remote` — inside Xmip's
mutual TLS, and takes `certificate`, `key` and `anchors` with it; without
it the node serves no network listener. With no `[operate]` at all the node
is operated at `xmip-<node>`, every caller an Observer:

```toml
[operate]
local = "xmip-node1"                # the local endpoint's name; xmip-<node> if left out
listen = "0.0.0.0:7401"             # host:port; no port is assumed
certificate = "tls/node.pem"        # this node's PEM chain, leaf first
key = "tls/node.key"                # its PEM private key
anchors = "tls/cluster-anchor.pem"  # what a surface's certificate must reach
role = "operator"                   # every proven caller's; Observer if left out
admit = ['CONTOSO\xmip-operators']  # who else may open the local endpoint
group = "xmip-operators"            # on Unix, the socket's group; the node's own if left out
```

A relative path is relative to the configuration file. Every caller on the
network is the subject of the certificate it presented, and on the local
endpoint the user its process runs as; `role` is what each may do, and a
word that is no role is Observer, which acts on nothing. A key it does not
know is refused. `xmip-service` refuses to start a node whose listener cannot
bind or whose files cannot be read, as it refuses any node that cannot
start.

### Where a node's certificates come from: `[provision]`

A node's certificates are obtained and renewed by `xmip-core-provision`
(ADR-0034; ADR-0033, amendment 2026-10-13), one table per purpose
(`XmipConfigurationDocument::provision`, each as written):
`[provision.receive]` for what a Party reaches — a Receive Location's server
certificate — and `[provision.node]` for node to node, where each end presents
one in mutual TLS and so needs server and client authentication. `source`
names the technology by its module name and the rest of the table is its
settings; a table with no `source` is the operator's own files. A certificate
is obtained again at two thirds of the life it was issued with, whatever that
life is. No source is a default; Let's Encrypt is one directory an `acme`
source may name, and serves `receive` only: it has issued no client
authentication since 2026-07-08, so between nodes the cluster's own
authority issues.

```toml
[provision.receive]                      # what Parties reach
source = "xmip-core-provision-acme"
directory = "https://acme-v02.api.letsencrypt.org/directory"
names = "partner-gateway.example"        # DNS names, comma-separated
agree_terms = true                       # the authority's terms; false unless said
challenge = "http-01"                    # or "dns-01"
challenge_listen = "0.0.0.0:8080"        # any port; public port 80 forwarded here

[provision.node]                         # between nodes
source = "xmip-core-provision-acme"
directory = "https://<authority-node>.example:9443/directory"
anchors = "tls/cluster-anchor.pem"       # the cluster authority's certificate
names = "<node>.example"
challenge_listen = "0.0.0.0:80"
```

| `source` | settings |
| --- | --- |
| none: the operator's files | `certificate` and `key` required, PEM, relative to this file; `anchors` optional |
| `xmip-core-provision-acme` | `directory` and `names` required; `challenge` `http-01` (default) or `dns-01`; `challenge_listen` (default `0.0.0.0:80`); `anchors`, the PEM the directory's TLS certificate must reach (absent, the operating system's trust store); `contact`; `agree_terms` (default false); `timeout` (default `10s`); `validation_wait` (default `2m`) |
| `xmip-core-provision-internal-ca` | `names`, `lifetime` and `authority_lifetime` required; `authority`, the folder it is kept in (default `authority`, beside this file); `serve`, host and port where it answers ACME for the premises; `publish`, host and port where it publishes its revocation list and certificate over plain HTTP; `published_at`, the `http://host:port` the premises reach that at, which every certificate it signs names; `challenge_port`, where it asks a node's http-01 answer (default 80); `dns_server`, host and port of the DNS server on the premises it checks dns-01 against |
| `xmip-core-provision-self-signed` | `names` and `lifetime` required: a lone offline node, its certificate its own anchor |

- **http-01** is asked on the name's port 80 by Let's Encrypt and on no
  other port; the node's answer listens wherever `challenge_listen` says,
  and one that listens elsewhere is reached from public port 80 through
  forwarding. A port below 1024 needs an account allowed to bind it.
- **dns-01** needs no port and suits a node behind a firewall; it writes the
  domain's TXT record through a DNS provider's module (`acme::DnsWriter`).
  Xmip ships none for a vendor, and a node choosing dns-01 with none linked
  is refused in words.
- **The cluster's authority** keeps its root in the purpose's place and
  issues to its own node directly; with `serve`, every other node names its
  directory, `https://<its first name>:<port>/directory`, and its certificate
  as `anchors`, and obtains its own by ACME like any other.
- **The premises' own ACME service** (ADR-0034, amendment 2026-10-22): with
  `serve`, `xmip-service` answers ACME there for every client on the
  premises — Xmip's nodes and any third party's, lego proved — with ACME
  Renewal Information (RFC 9773) and revocation, until the node drains, and
  with `publish` its revocation list (`/crl`) and certificate
  (`/authority.cer`) over plain HTTP; `xmip-service --listening` says both
  ports. Every surface sets it up, inspects, revokes from, exports and
  backs up the folder `authority` names (`xmip certificate`,
  `Get-XmipCertificate`).
- **A node's certificate always comes from the cluster's authority**: a
  `[provision.node]` whose source trusts the operating system's store — a
  public authority — is refused, so `anchors` names the cluster
  authority's anchor. Let's Encrypt serves `[provision.receive]`, the
  public server side, alone (the owner, 2026-10-22).

**A person's certificate** — purpose `operator`, client authentication
alone, its subject the person — is never obtained by a node: the person's
surface makes its own key and a request, and the node whose authority
signs it says where that authority is kept and how long the certificate
lasts (ADR-0034, amendment 2026-10-15). The table names no `source`:

```toml
[provision.operator]                     # on the node whose authority signs
authority = "authority"                  # holds authority.key and authority.pem; relative to this file
lifetime = "720h"                        # thirty days unless said
```

`xmip certificate <request> --sign <this file>` and
`New-XmipCertificate -Request <request> -Configuration <this file>` sign by
it; a request asking more than client authentication for a person is
refused.

A table is read through its technology's declaration
(`provision::read`): an unknown purpose, a `source` the program does not
link — naming those it does — an unknown key, a value of the wrong kind and
a required setting left out are each refused, naming the setting. Which
sources a node can use is its program's. `xmip-service` reads
`[provision.receive]` as it starts, from the operator's own files alone, and
hands it to every transport as the TLS its receives terminate with; it
starts no provisioner, so a table naming a `source` is refused. It reads
`[provision.node]` only to serve the cluster's authority where it names
`internal-ca` and `serve`: certificates still reach a node as files under
`[operate]` and the Storage tables (open problem 27), carried across an air
gap where no route reaches the authority (`xmip certificate --node`,
`--import`).

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
receive_poll = "1s"                         # a polled Location's rest after finding nothing
receive_sweep = "1s"                        # a watched Location's recovery sweep
receive_backoff = "5s"                      # a polled Location's rest after a failure
receive_drain = "1s"                        # the longest a stopping Location drains
receive_handshake = "5s"                    # each TLS handshake read, apart from a Location's wait
node_receive_threads_per_hardware_thread = 4 # 1 to 256
receive_chunks_per_thread = 8               # 1 to 4096
send_threads_per_hardware_thread = 2        # 1 to 64
send_idle = "1m"                            # a send thread's idle time
send_lease = "30s"                          # how long a claim on a Journey holds
send_scan = "1s"                            # how often the send queues are read
failed_window = 100                         # oldest failed Journeys kept per Send Port, 1 to 10000
send_chunks_per_thread = 16                 # 1 to 65536
stream_stall = "100ms"                      # how long a send taking nothing holds its Stream's others back
storage_timeout = "5s"                      # each connect and read to a Storage node
storage_pass_over = "5s"                    # how long one that did not answer waits its turn
local_timeout = "2s"                        # how long a call to the local endpoint waits
```

- **`tcp_segment`** and **`segments`**: a Stream is written to the Ledger
  in chunks of `segments` × `tcp_segment` bytes, 64,240 by default
  (`runtime-model.md` section 3).
- **`receive_threads_per_hardware_thread`** and **`receive_idle`**: a
  Receive Location's pool grows to this many threads per hardware thread
  the machine runs, and a thread with nothing to do ends after its idle
  time.
- **`receive_poll`** and **`receive_backoff`**: how long a Receive
  Location whose transport is polled — its receive looks and returns at
  once, a directory listed, a query run (`transport::Waiting`) — rests
  before it asks again after a receive that found nothing, and after one
  that failed in a way asking again may mend, each measured from when that
  receive began; a stop ends the rest at once. A Location whose receive
  waits for what arrives is asked again at once (`runtime-model.md`
  section 5, *How a receive runs*).
- **`receive_sweep`**: how long a polled Receive Location whose far end
  tells it of a change — a file Receive Location's drop directory, watched
  by the operating system's change notification (`transport::Notice`) —
  rests after a receive that found nothing when no word comes: its
  recovery sweep, for a file that lay there before the watch began or a
  word missed. The word ends the rest at once (ADR-0024, amendment
  2026-10-20).
- **`receive_drain`**: how long, from the node's stop, a Receive Location
  whose receive waits and whose far end let go of what it handed over —
  raw TCP, a datagram — goes on taking what is already queued for it; it
  stops sooner at the first receive that finds nothing (`runtime-model.md`
  section 5, *A stop drains what the far end already let go of*).
- **`receive_handshake`**: the most each read of a TLS handshake a Receive
  Location terminates waits on the client (`tls = true`). It is the
  guard's, apart from the Location's own `timeout`, which bounds the
  content and is put back before a byte of it is read, so a Location's
  wait is never raised for a slow handshake (ADR-0033, amendment
  2026-10-20).
- **`node_receive_threads_per_hardware_thread`** and
  **`receive_chunks_per_thread`**: the node's budget beside each Receive
  Location's own pool. The node carries at most this many arrivals per
  hardware thread at once across all its Receive Locations, and its carries
  hold at most `receive_chunks_per_thread` chunks of Streams each, on
  average; a carry is counted as holding eight — the chunk it writes, the
  one it reads ahead, and what Xmip Storage encodes and seals of it,
  measured at seven at most — so by default the carries bound the bytes. A receive takes no more arrivals than its Location's pool and the
  node have room for, and what it does not take stays at its far end
  (`runtime-model.md` section 3, *A node's budget beside its pools*).
- **`send_threads_per_hardware_thread`** and **`send_idle`**: the same for
  the node's one Send pool (`runtime-model.md` section 3).
- **`send_chunks_per_thread`**: the bytes the node's sends hold at once,
  in chunks for each thread of the Send pool, beside the pool's count of
  Journeys. A send whose transport streams is counted as holding six
  chunks, what reading it holds, measured at a little over five; one
  whose transport gathers the whole Message holds all of it, and waits for
  room where others hold the rest (`runtime-model.md` section 10).
- **`stream_stall`**: how long a send of a fan-out that takes no chunk of
  its Stream holds the sends of the same Stream ahead of it back before
  they leave it behind, to read what it missed from Xmip Storage once it
  takes again. While it keeps taking it is waited for, however slowly
  (ADR-0018, amendment 2026-10-27). The 100 ms is a draft for the owner.
- **`send_lease`** and **`send_scan`**: how long a claim on a Journey holds
  before it lapses unless renewed, and how often the queues of the Send
  Ports this node sends are read for unclaimed Journeys (`runtime-model.md`
  section 10).
- **`failed_window`**: how many of the oldest failed Journeys of each Send
  Port the node keeps and publishes. Xmip Storage keeps the count and every
  one of them in the Ledger, in each queue's index of failed Journeys; a
  surface pages the rest from there, so the node's memory stays bounded
  however many fail (`runtime-model.md` section 10, corrected 2026-10-16).
- **`storage_timeout`** and **`storage_pass_over`**: what bounds each
  connect to and read from a Storage node, and how long one that did not
  answer — or said it is a standby — is asked only after the rest
  ([built, in the assembled service](../../../../doc/architecture/estate-map.md#storage-nodes)).
- **`storage_beat`**, **`storage_silence`** and **`storage_kept`**: on a
  Storage node one of several, how often a primary says it is there to an
  idle standby (100 ms), how long a standby hears no primary before it is
  promoted (1 s), and how many bytes of each database's writes a primary
  keeps for a standby behind it (64 MiB); one further behind is copied whole
  ([built, in the assembled service](../../../../doc/architecture/estate-map.md#storage-standby)).
- **`local_timeout`**: how long a surface's call to the node's local
  endpoint waits — decided by the owner, 2026-10-13: *a local call waits at
  most a set time — `[tuning]`, default 2 s — then says the node did not
  answer and shows it so; the tool recovers on its own.* The node declares
  it beside its endpoint (`local_timeout_ms`, ADR-0053), and a surface on
  the machine that names no wait of its own takes it from there.

Durations are a whole number and `ms`, `s`, `m` or `h`, above nothing and
at most an hour. Which keys there are, their kinds, bounds and defaults are
the runtime's one declaration (`xmip-core-runtime`'s `tuning::TUNING`, in
`xmip-core`'s settings shape), and the table is read through it as a
technology reads a Location's settings: an unknown key, a value of the
wrong kind or outside its bounds is refused at startup phase 3, and by
`xmip_validate_v1`, in words.

### Where a node reaches Xmip Storage

Every node calls Xmip Storage — the nodes declaring the Storage role — for
all storage, never a database directly; a node without the role finds them
from a list of their addresses, tried round robin (`deployment-model.md`
section 7; `src/storage.rs`):

```toml
[storage]
nodes = ["storage-1.example:7443", "storage-2.example:7443"]
listen = "0.0.0.0:7443"
certificate = "tls/node.pem"
key = "tls/node.key"
anchors = "tls/anchors.pem"
```

- **`nodes`**: each `host:port`, a host name or an address, IPv6 in
  brackets, each once. More than one is the safety: a node carries on
  through the next while one is stopped. An address without its port is
  refused in words, and so is a Storage node named twice. A node declaring
  the Storage role that lists them is one of several, which every Storage
  node of the cluster lists in the same order: the first listed is the first
  primary, the others its standbys, and a standby is promoted once the
  primary is gone (ADR-0056, amendment 2026-10-22).
- **`listen`**: where a node declaring the Storage role serves Xmip Storage
  to the nodes reaching it, the node's address and the Storage plane's port
  (ADR-0067, amendment 2026-10-22); read by such a node alone, so one
  cluster-wide `[storage]` serves every node. One of several that names none
  is refused as it starts.
- **`certificate`**, **`key`** and **`anchors`**: all three or none — what
  the node presents over Xmip's mutual TLS to the Storage nodes it reaches,
  and serving to the nodes reaching it, and the authority the other end's
  certificate must reach, the cluster's own (`New-XmipCertificate -Node`).
  Paths are relative to the configuration file. A node listing Storage
  nodes and naming none of them is refused as it starts, in words.

### Where each data domain is kept: `[runtime]`, `[administration]`, `[audit]`

Xmip Storage keeps three databases, one to each data domain, and each is a
table of its own, its `storage` and its `connection` (`src/database.rs`;
the owner, 2026-10-10: *i would do it like runtime, storage, connection
string. Same for audit and administration*, and *Yes, better*). Each may be
on another technology and another server:

```toml
[runtime]
storage    = "postgresql"
connection = "host=db-1.example port=5432 dbname=xmip_runtime user=xmip_storage"

[administration]
storage    = "mssql"
connection = "Server=tcp:sql-1.example,1433;Database=xmip_administration;User Id=xmip_storage"

[audit]
storage    = "sqlite"
connection = "D:/Xmip/data/storage/audit.sqlite"

[storage.database]
password     = "xmip-storage-database"       # a secret's name, never the password
trust_anchor = "../config/database-authority.pem"
```

- **`storage`**: `rocksdb`, `sqlite`, `postgresql`, `mssql` or `mariadb`.
  Embedded, the runtime database is `rocksdb` and the administration and
  audit databases `sqlite` (ADR-0015, amendment 2026-10-01).
- **`connection`**: on an embedded engine, the store's path, relative to
  this file; on a database server, the server's own connection string —
  `host=<host> [port=<port>] dbname=<database> [user=<login>]` for
  PostgreSQL, `Server=[tcp:]<host>[,<port>];Database=<database>[;User
  Id=<login>]` for SQL Server, `Server=<host>[;Port=<port>];
  Database=<database>[;User=<login>]` for MariaDB, the port the server's
  own (5432, 1433, 3306) where it is left out. Nothing else is written in it: no password, no option.
  Two domains naming one database on one server are refused: the three are
  separate.
- **A table left out** is the embedded Storage node's own, under the data
  directory: the runtime database `rocksdb` at `<data>/storage/runtime`,
  the administration database `sqlite` at
  `<data>/storage/administration.sqlite`, the audit database `sqlite` at
  `<data>/storage/audit.sqlite`.
- **`[storage.database] password`**: where a domain names a database
  server, the name of the secret the login's password is kept under,
  resolved through the key home (ADR-0063 clause 4); the password is never
  written here.
- **`[storage.database] trust_anchor`**: the authority the server's
  certificate reaches, PEM, relative to this file; absent, the operating
  system's trust store. Xmip always speaks TLS to the database server.

A database server's backend is not built yet: a node naming one for a
domain is refused as it starts, in words. What a site's IT operators
install and run for it — the software, the scripts that make each
database and its roles, the settings Xmip depends on — is
`deploy/database/postgresql/README.md`,
`deploy/database/mssql/README.md` and
`deploy/database/mariadb/README.md` at the estate root.

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

### Compiled units: Prepare, Promote, Demote, Transformation and a flow

Prepare, Promote, Demote, a Transformation and a Work Process's flow are
compiled units a Module provides, and the configuration refers to them
(ADR-0066, amendment 2026-10-22; the owner: *They are compiled units. They
just have to be referred to in the VS Code extension, ending up in the TOML
configuration.*). A unit is a function; a step entry is a call to it with
arguments:

```toml
# a complete node file
roles = "executing, storage"

[service]
name = "xmip-<node>"
cluster_name = "<cluster>"
node_name = "<node>"

[[modules]]
name = "codecs"
start = true
[modules.manifest]
capabilities = []
offline = true                        # required: Xmip uses only what works offline
[modules.manifest.identity]
name = "codecs"
version = "1.0.0"                     # the version the configuration pins
[modules.manifest.entrypoint]
library_path = "codecs.dll"
[[modules.manifest.units]]
name = "Codec"
capability = "prepare"                # prepare, promote, demote, transform, work-process, validate, debatch
stages = ["receive", "send"]          # handed the one it is called at
offline = true
[[modules.manifest.units.settings]]
name = "format"
kind = "choice"
choices = ["gzip", "zstd"]
presence = "default"
default = "gzip"
meaning = "How the Stream is compressed."

[[preparation_steps]]
name = "Unzip"
module = "codecs"
unit = "Codec"
settings = { format = "gzip" }

[[receive_locations]]
name = "drop"
start = true
transport = "xmip-core-transport-file"
address = "/var/xmip/in"
prepare = ["Unzip"]                   # Preparation Steps, in order
[receive_locations.accept]
mechanism = ["circumstance"]
```

- **A unit** is declared in its Module's manifest: its name, its
  capability — never read from its name — the stages it serves, `receive`,
  `process` and `send` in the message path's own words, that it works
  offline, and what a call may give, each setting with its kind, presence,
  default, words and meaning, as a technology declares a Location's
  settings. Promote serves `receive` alone, Demote `send` alone, and
  Debatch `receive` and `process` alone (ADR-0066, amendment 2026-10-27). A
  prepare unit at a stage is a Receive, Process or Send Preparation; a
  Transformation is never a Preparation.
- **A call** is an entry of `[[preparation_steps]]`, `[[promotions]]`,
  `[[demotions]]`, `[[transformations]]` or `[[debatches]]`, here or in an Xmip
  Application's section: `name`, `module`, `unit` — of the list's
  capability — and `settings`, read through the unit's declaration. A Work
  Process names its flow with `module`, `unit` and `settings`, a
  `work-process` unit serving `process`.
- **An attachment** names calls of its own document: `prepare = [...]`, in
  order, `transform`, `promote` and `debatch` on a Receive Port or Location
  and `demote` on a Send Port or Location; receive runs the Location's, then
  the Port's, send the Port's, then the Location's. A unit is attached only
  at a stage it serves. A Debatch splits the received Message after Prepare
  and Message creation, before the Contract, all or nothing, its records
  routed in their inbound order once every one is split and validated
  (ADR-0031, amendment 2026-10-27).
- **Refused in words** by `xmip_validate_v1` and as a node starts
  (`step::problems`): a module or unit that does not say `offline = true`,
  a module no `[[modules]]` declares, a unit its manifest does not declare
  or declares as another capability, a stage that is none or listed twice,
  settings the unit's declaration refuses, an attachment naming no call, a
  unit attached at a stage it does not serve, a Promotion or a Debatch on
  send and a Demotion on receive. Running a unit is [decided, not built](../../../../doc/architecture/estate-map.md#artifact-steps):
  a node refuses to start a Port or Location that attaches one.
- **Assignment is not configured**: a Work Process's compiled code assigns
  (the owner, 2026-10-22).

### What a Receive Location accepts

A Receive Location declares the **closed set** it authenticates (ADR-0019
clause 1): the mechanisms, each by the name its mechanism declares, and,
where it takes named Parties alone, those Parties, each by its name in
`[[parties]]`:

```toml
[[parties]]
id         = "<a UUIDv7, minted once with the Party>"
name       = "<party>"
identities = [
  { mechanism = "mutual-tls", purpose = "receive", value = "CN=<party>.example" },
]

[receive_locations.accept]
mechanism = ["mutual-tls", "oauth2"]
party     = ["<party>"]
```

**A Party is written with its `id`**, a UUIDv7 minted once when the Party
is written and never derived from where it stands in the list (the owner,
2026-10-19): it is what a verified identity resolves to, what a filter's
`xmip.party` holds and what the audit records. Every surface that writes a
Party writes one — the designer mints it as it adds the Party — and a
Party without one, or with one that is not a UUIDv7 or is another Party's,
is refused in words as the node starts. **Each identity states its
mechanism and its purpose** (the owner, 2026-10-19; ADR-0019 clause 4): the
mechanism that verifies it, by the name it declares, and whether it is for
`receive` — matched against what arrives — or `send` — what Xmip presents.

An identity presented by any other mechanism is refused at authentication,
and never tried against the rest. A verified identity resolves to the Party
holding its value under the mechanism that verified it, for `receive`;
where `party` is given, one resolving to another Party, or to none, is
refused at authentication too, and audited with its claim. A Receive
Location that gives no `accept` takes nothing — an unconfigured endpoint is
closed, not open — and one receiving from something that presents nothing,
a drop folder or a raw socket, accepts `circumstance`: the circumstance is
the transport identity, implied by where the Stream came from, and every
build of `xmip-service` verifies it (ADR-0019 clause 7; the owner,
2026-10-19). A node that starts one refuses a mechanism no authenticator it
was built with verifies, and a Party no `[[parties]]` declares. A Send
Location presents and accepts nothing, so `accept` on one is refused. Where
the node is built with the `location` policy, each Receive Location it
starts admits what its `accept` takes, and each Send Location it starts is
admitted for what reached it (the owner, 2026-10-19); without a policy,
nothing is permitted.

**A file Send Location's address names a directory**, and each Message is
written there under the name it was received under, `file.name` in its
context, which a file Receive Location writes (ADR-0024, amendment
2026-10-20); a Message carrying none is written under the pattern the Send
Location's `name` setting gives, `{message}` in it the Message's
identifier, or under the identifier alone where it gives none (the owner,
2026-10-21; ADR-0024, amendment 2026-10-21):

```toml
[applications.send_ports.settings]
name = "order-{message}.edi"
```

**`mutual-tls` over tcp.** A tcp Receive Location whose settings say
`tls = true` terminates TLS on every connection: it presents the node's
`[provision.receive]` certificate and, where that table gives `anchors`,
refuses a client whose certificate does not reach them before a byte of
content is read. What the handshake proved — the certificate's subject,
issuer and fingerprint — is what the first gate claims as `mutual-tls`. A
Location saying `tls = true` on a node with no `[provision.receive]` is
refused as the node starts.

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
desktop editor, the language server, `xmip validate`,
`Test-XmipNodeConfiguration` — asks the runtime rather than judging for
itself. The desktop editor saves a half-built document so you can keep work in
progress, and shows the runtime's verdict on it.

---

