# MeshMind: A Peer-to-Peer LLM Network

## Table of Contents
1. Project Overview
2. Problem Statement
3. Solution Approach
4. System Architecture
   4.1 High-level Components
   4.2 Data Flow
   4.3 Networking and Ports
5. Implementation Details
   5.1 Backend (Rust/Actix)
   5.2 P2P Layer (UDP/TCP)
   5.3 LLM Integration (Ollama + Peer Delegation)
   5.4 Persistence and Files
   5.5 Web UI
6. Security and Authentication
7. Key Features
8. APIs and Routes
9. Installation and Setup
10. Troubleshooting
11. Evaluation Methodology (Latency, Availability, Bandwidth, Resource Usage)
12. Future Enhancements
13. Conclusion

## 1) Project Overview
MeshMind is a LAN-first, peer‑to‑peer network that federates LLM access, shared files, and conversation history across machines—no cloud required. Any node can act as an LLM host (via Ollama) or as a thin client that transparently delegates chat requests to the most capable reachable peer. The system emphasizes privacy (local compute), simplicity (single Rust binary embedding the UI), and resilience (no single point of failure).

## 2) Problem Statement
- Centralized LLM services raise cost, privacy, and availability concerns.
- Sharing compute/resources across nodes is complex without a standard substrate.
- Real-time collaboration (files, conversations) needs reliable discovery and transport.
- Security and trust in a local P2P environment require lightweight controls.

## 3) Solution Approach
- UDP broadcast for auto‑discovery of peers on the LAN.
- TCP links for control messages and file propagation.
- HTTP server (Actix Web) for a same-origin Web UI and APIs.
- Optional LLM hosting via Ollama; automatic remote delegation when local LLM is absent.
- Local persistence for files and conversations; aggregated views across peers.

## 4) System Architecture

### 4.1 High‑level Components
- Backend (Rust/Actix):
  - Hosts `/app` UI and `/api/*`.
  - Orchestrates discovery, P2P links, file aggregation, auth, and LLM usage.
- P2P Layer:
  - UDP discovery (port 5000).
  - TCP control/file channel (port 7878).
- LLM Integration:
  - Local Ollama detection (http://127.0.0.1:11434).
  - Remote LLM usage over peer `/api/chat`.
- Web UI:
  - React + Tailwind assets embedded in the binary.

### 4.2 Data Flow
1. Discovery: peers broadcast ONLINE messages over UDP/5000, maintaining a peer set with last-seen timestamps and timeouts.
2. Control + Files: TCP/7878 links exchange conversation files and metadata; also announce LLM capability and respond to LLM access requests.
3. Aggregation/UI: HTTP/8080 endpoints serve the web app and APIs. File lists are merged from local store, received peer files, and live peer queries. Same‑origin proxy fetches peer files without CORS/cookie issues.
4. Chat: A node checks for local Ollama; if absent/unavailable, it delegates to a connected peer that exposes `/api/chat`.

### 4.3 Networking and Ports
- 8080/TCP: Actix Web server (APIs + UI)
- 7878/TCP: P2P control and file propagation
- 5000/UDP: Peer discovery and announcements
- 11434/TCP (local only): Ollama (LLM host)

## 5) Implementation Details

### 5.1 Backend (Rust/Actix)
- Launches HTTP server on `0.0.0.0:8080`.
- Middleware:
  - Auth guard for `/api/*` and `/peers`, except internal peer calls identified by header `x-peer-llm: 1` for read-only operations and LLM chat proxying.
  - Performance metrics middleware tracking per-route latencies and totals.
- Embedded static UI via `rust-embed`.
- Spawns background tasks:
  - UDP receive loop (peer discovery).
  - UDP periodic broadcaster.
  - TCP listener for incoming peer connections.
  - TCP connector that dials newly discovered peers.

### 5.2 P2P Layer (UDP/TCP)
- UDP (`src/udp/mod.rs`):
  - Broadcasts ONLINE messages every 30s; includes `has_llm` and timestamp.
  - Receives and records peers with last-seen timestamps; debounces noisy logs.
  - Checks local Ollama (`/api/tags`) to advertise `has_llm`.
- TCP (`src/tcp/mod.rs`):
  - Maintains sets: LLM‑capable peers, authorized peers, and active streams.
  - On connect/accept:
    - Announces LLM capability.
    - Shares local conversation as `local.json`.
    - Stores active streams for broadcast.
  - Handles:
    - `ConversationFile { name, content }`
    - `LLMCapability { has_llm }`
    - `LLMAccessRequest/Response` with host IP and port 8080 when LLM is available.
  - Tracks connected peers, creates per‑peer directories under `received/<peer-ip>/`.

### 5.3 LLM Integration (Ollama + Peer Delegation)
- Chat endpoint (`POST /api/chat` in `src/llm/mod.rs`):
  - Composes an Ollama chat request (default model: `llama2`) with a system prompt tuned for file/PDF analysis.
  - If a filename is provided, reads local storage:
    - For PDFs or binary, provides base64 previews to the prompt (8KB).
    - For text, includes up to 4,000 characters as preview.
  - Saves the user question and the LLM response to the conversation store with host info (hostname, LAN IP, LLM availability).
- Local vs Remote:
  - Checks local Ollama via `/api/tags` on `127.0.0.1:11434`.
  - If local succeeds, uses it; else tries connected peers’ `/api/chat` with `x-peer-llm: 1`.
  - Also supports processing Ollama’s streaming JSON lines by concatenating message chunks until `done=true`.

### 5.4 Persistence and Files
- Local uploads and received files are persisted.
- Conversation store maintains local and per‑peer histories (e.g., `received/<peer-ip>/local.json`).
- File listing endpoint merges:
  - Local uploads,
  - Received files,
  - Live peer lists via HTTP with `x-peer-llm: 1`, with throttling and de-duplication.

### 5.5 Web UI
- React + Tailwind + Framer Motion (embedded into the Rust binary).
- Screens:
  - Dashboard (LLM host indicator, quick actions)
  - Peer Conversations
  - Shared Files (aggregated list + proxy downloads)
  - Analytics (engagement, performance, network)
  - Auth (login/logout)
- Served at `http://localhost:8080/app/`.

## 6) Security and Authentication
- Session cookie: HS256 JWT (24h), HttpOnly, Lax same-site.
- Internal peer calls:
  - Header `x-peer-llm: 1` grants read-only access to file endpoints and allows chat relay between peers without user cookie.
- P2P trust:
  - Shared secret (HMAC) for announcements/metadata (configured via env/file).
- Same-origin proxy:
  - `/api/peer-file/{ip}/{filename}` fetches peer files server-side to avoid exposing cross-origin cookies.

## 7) Key Features
- Automatic peer discovery on LAN (UDP 5000)
- Resilient TCP links (7878) with conversation/file propagation
- Shared LLM access via most capable reachable host (Ollama integration)
- Conversation memory (local + peer)
- Aggregated file system view with proxy downloads
- Secure session auth and lightweight peer trust
- Built as a single Rust binary with embedded UI

## 8) APIs and Routes (selected)
- Auth:
  - `POST /api/auth/login`, `POST /api/auth/logout`, `GET /api/auth/status`
- Status:
  - `GET /api/status` → `{ is_llm_host, peer_count }`
- Files:
  - `POST /api/upload` (multipart field `file`)
  - `GET /api/files` (aggregated list; requires session or `x-peer-llm`)
  - `GET /api/files/{filename}` (local download)
  - `GET /api/peer-file/{ip}/{filename}` (same-origin proxy)
- Conversations / Peers:
  - `GET /peers`, `GET /api/local`
- LLM:
  - `POST /api/chat` (handles local/remote LLM; accepts optional `filename`)
- Analytics (summaries in code):
  - `/api/analytics/*` endpoints for chat, files, engagement, perf, network

## 9) Installation and Setup
Requirements:
- Rust (stable), Node.js (only to develop UI; binary embeds assets)
- Windows or Linux
- Optional: Ollama on nodes that will host the LLM

Host:
```bash
cargo build --release
cargo run --release
# UI: http://localhost:8080/app/
```

Peer (Option A: build on peer):
```bash
git clone <repo>
cargo run --release
```

Peer (Option B: prebuilt binary on Windows):
1) Copy `target/release/instance.exe` and `p2p_secret.txt` (same as host) to a folder, e.g., `C:\MeshMind\`
2) Open peer inbound firewall (see below)
3) Run:
```powershell
cd C:\MeshMind
./instance.exe
```

Configuration:
- `P2P_HMAC_SECRET` env var or `p2p_secret.txt` file (same on all nodes)
- `NODE_USERNAME` / `auth_user.txt`; `NODE_PASSWORD` / `auth_secret.txt`
- Default ports: 8080 (HTTP), 7878 (TCP), 5000 (UDP)

Windows firewall (examples):
```powershell
New-NetFirewallRule -DisplayName "MeshMind HTTP 8080" -Direction Inbound -Protocol TCP -LocalPort 8080 -Action Allow -Profile Private,Public
New-NetFirewallRule -DisplayName "MeshMind P2P TCP 7878" -Direction Inbound -Protocol TCP -LocalPort 7878 -Action Allow -Profile Private,Public
New-NetFirewallRule -DisplayName "MeshMind UDP 5000" -Direction Inbound -Protocol UDP -LocalPort 5000 -Action Allow -Profile Private,Public
$exe = "C:\\Users\\Lenovo\\Documents\\VIT\\TY\\Sem 5\\CN\\LLM-Network\\target\\release\\instance.exe"
New-NetFirewallRule -DisplayName "MeshMind instance.exe OUT 8080" -Direction Outbound -Protocol TCP -RemotePort 8080 -Action Allow -Program $exe -Profile Any
New-NetFirewallRule -DisplayName "MeshMind instance.exe OUT LAN"  -Direction Outbound -Action Allow -Program $exe -RemoteAddress 192.168.0.0/16 -Profile Any
```

## 10) Troubleshooting
- 0 peers or empty file list:
  - Ensure peer is running: `Test-NetConnection <peer-ip> -Port 8080`
  - Host must allow outbound 8080 for `instance.exe`
  - Peer must allow inbound 8080/7878/5000
  - `p2p_secret.txt` must match on all nodes
- 401 on `/api/files`:
  - Use header `x-peer-llm: 1` or login to obtain a session cookie
- Logs too fast:
  - `cargo run --release 2>&1 | Tee-Object -FilePath host.log`
  - Remote listing fetches are throttled (~15s TTL)

## 11) Evaluation Methodology
- Latency:
  - Measure p50/p95/p99 for `/api/files`, proxy downloads, and chat.
- Availability:
  - Under peer churn; compute success ratios and time-to-recovery.
- Bandwidth:
  - Estimate UDP discovery overhead, TCP propagation, proxy downloads.
- Resource usage:
  - CPU/memory at idle and under load (Task Manager/PerfMon).

## 12) Future Enhancements
- Dynamic model selection/orchestration.
- Peer validation and trust scoring; rotating HMAC secrets.
- Encrypted query/result sharing; differential privacy.
- Decentralized cache/gossip of indexes and embeddings.
- WAN overlay with NAT traversal; auto-update peers.

## 13) Conclusion
MeshMind demonstrates a practical, research-ready P2P substrate for sharing LLM capabilities and artifacts across a LAN. It combines auto-discovery, resilient transport, secure session handling, and a same-origin UI into a single Rust binary with optional Ollama integration. The design favors privacy, resilience, and ease of deployment while enabling future research into federated reasoning and collaborative AI.