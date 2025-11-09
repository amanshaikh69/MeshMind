# MeshMind

MeshMind is a distributed, peer‑to‑peer (P2P) network for sharing LLM capabilities, files, and conversations across nodes on a LAN. Any node can act as an LLM host (with Ollama) or a lightweight client. Peers are discovered automatically; metadata and binaries are exchanged securely, and a same‑origin web UI provides a seamless experience.

## Table of Contents

- [Abstract](#abstract)
- [Introduction](#introduction)
- [System Architecture](#system-architecture)
  - [Component Diagram](#component-diagram)
  - [Data Flow](#data-flow)
  - [Networking and Ports](#networking-and-ports)
- [Implementation Details](#implementation-details)
  - [Backend (Rust/Actix)](#backend-rustactix)
  - [P2P Layer (UDP/TCP)](#p2p-layer-udptcp)
  - [Frontend (React/Tailwind/Framer Motion)](#frontend-reacttailwindframer-motion)
  - [Security and Auth](#security-and-auth)
- [Key Features](#key-features)
- [Research & Innovation Aspects](#research--innovation-aspects)
- [UI/UX Design Rationale](#uiux-design-rationale)
- [Installation and Setup](#installation-and-setup)
  - [Requirements](#requirements)
  - [Host Setup](#host-setup)
  - [Peer Setup](#peer-setup)
  - [Configuration](#configuration)
  - [Windows Firewall Guidance](#windows-firewall-guidance)
- [Troubleshooting](#troubleshooting)
- [Future Scope](#future-scope)
- [How to Cite](#how-to-cite)
- [References](#references)

## Abstract

MeshMind is a decentralized AI assistant system that federates local Large Language Model (LLM) resources across a mesh of peers. Each peer can host an LLM (via Ollama) or operate as a thin client that discovers and securely delegates queries to reachable LLM hosts. Beyond inference, MeshMind synchronizes conversation context and aggregates shared files across the network, exposing a single, same‑origin web interface. The system emphasizes privacy (local compute), resilience (no single point of failure), and practical deployment (single Rust binary with embedded UI). We detail a hybrid design that combines UDP discovery, TCP control, and HTTP proxying to enable secure and usable distributed AI on commodity LANs.

## Introduction

Centralized, cloud‑hosted AI assistants concentrate compute and data, raising cost and privacy concerns. MeshMind explores a complementary direction: decentralized, LAN‑first AI assistance where nodes collaborate to share LLM capability and knowledge artifacts. The goal is a practical, research‑ready platform for studying federated reasoning, collaborative memory, and resilient AI service delivery without requiring centralized infrastructure.

## System Architecture

MeshMind comprises an Actix Web backend with embedded UI assets, a UDP/TCP P2P substrate, and a React/Tailwind/Framer Motion frontend. The backend hosts HTTP APIs and orchestrates discovery, file aggregation, auth, and optional LLM execution. Peers announce themselves over UDP, connect over TCP for control/file propagation, and expose a read API over HTTP for aggregation.


### Data Flow

1. Discovery: peers periodically broadcast presence over UDP/5000 and maintain a set of known IPs.
2. Control/File path: TCP/7878 is used for peer links and binary propagation.
3. Aggregation (host): HTTP/8080 endpoint `/api/files` merges
   - local uploads,
   - received binaries under `received/<peer-ip>/`, and
   - live peer file lists via `GET http://<peer>:8080/api/files` with header `x-peer-llm: 1`.
4. Proxy download: same‑origin proxy `/api/peer-file/{ip}/{filename}` fetches from a peer and returns bytes to the browser, avoiding cross‑origin cookies.
5. Conversations: local and per‑peer histories are loaded from disk (`received/<peer-ip>/local.json`) and exposed via `/peers` and `/api/local`.

### Networking and Ports

- 8080/TCP: HTTP API + Web UI (Actix Web)
- 7878/TCP: P2P control and file propagation
- 5000/UDP: Discovery broadcasts and announcements

## Implementation Details

### Backend (Rust/Actix)

- Actix Web server hosting `/app` and `/api/*` routes; static UI embedded via `rust-embed`.
- Authentication: username/password → HS256 JWT session cookie. Non‑UI peer calls are authorized by internal header `x-peer-llm: 1` for read‑only file APIs and proxy.
- File service:
  - `POST /api/upload` (multipart field `file`, max 50 MB)
  - `GET /api/files` (aggregated listing with de‑duplication and throttled remote fetch)
  - `GET /api/files/{filename}` (local download)
  - `GET /api/peer-file/{ip}/{filename}` (same‑origin proxy to peer)
- Analytics endpoints summarize performance and usage.

### P2P Layer (UDP/TCP)

- UDP/5000 periodic broadcast and receiver maintain known peers.
- TCP/7878 listener and connector handle control, HMAC validation, and file propagation.
- Announcements and metadata are signed with a shared HMAC secret (`P2P_HMAC_SECRET` or `p2p_secret.txt`).
- Remote file list fetch is throttled in‑process to avoid log spam and unnecessary traffic.

### Frontend (React/Tailwind/Framer Motion)

- Built with Vite (React + TS), TailwindCSS for consistent design tokens, and Framer Motion for smooth micro‑interactions.
- Key screens/components:
  - Dashboard (LLM Host indicator, actions)
  - Peer Conversations (per‑peer history)
  - Shared Files (aggregated file panel with proxy downloads)
  - Analytics (engagement, performance)
  - Auth (login/logout)
- Professional dark theme prioritizing contrast, motion hierarchy (ease, duration, stagger), and reduced cognitive load.

### Security and Auth

- Session cookie: HS256 JWT with 24h expiry (Lax same‑site, HttpOnly).
- Internal peer calls: header `x-peer-llm: 1` whitelists read‑only file endpoints and proxy.
- HMAC: shared secret authenticates peer announcements and file metadata.
- Same‑origin proxy prevents exposing peer cookies/CORS complexities.

## Key Features

- Peer‑to‑peer communication (UDP discovery, TCP links)
- Shared file system and aggregation across peers
- LLM host delegation via Ollama detection and remote usage
- Conversational memory (local + peer histories)
- Analytics dashboards for usage and performance
- Secure authentication and session management

## Research & Innovation Aspects

- Decentralization vs. cloud: private, local compute; no central dependency.
- Scalability: additional peers increase available compute and storage; discovery and aggregation are incremental.
- Fault tolerance: no single point of failure; peers still function with partial connectivity.
- Ethics and privacy: on‑prem data, controllable trust via shared secrets; enables research on policy and consent in collaborative AI.
- Research directions: federated prompt/result sharing, distributed reasoning across peers, gossip‑based model selection.

## UI/UX Design Rationale

- Professional dark theme enhances readability and energy use on OLED.
- Tailwind ensures consistent spacing, color, and typography scales.
- Framer Motion adds accessible motion (reduced motion friendly), emphasizing hierarchy (list to detail, panel transitions, subtle hover states).
- Same‑origin navigation keeps mental model simple; proxy makes cross‑peer actions feel local.

## Installation and Setup

### Requirements

- Rust (latest stable)
- Node.js (only for UI development; release binary embeds assets)
- Windows or Linux (Windows guidance provided)
- Optional: Ollama on nodes that will host LLM

### Host Setup

```bash
cargo build --release
cargo run --release
# UI: http://localhost:8080/app/
```

### Peer Setup

Option A – Build on peer:
```bash
git clone <this repo>
cargo run --release
```

Option B – Run prebuilt binary:
1) Copy `target/release/instance.exe` and `p2p_secret.txt` (same as host) to `C:\MeshMind\` on the peer.
2) Run:
```powershell
cd C:\MeshMind
./instance.exe
```

### Configuration

- `P2P_HMAC_SECRET` env var or `p2p_secret.txt` (identical on all nodes)
- `NODE_USERNAME` / `auth_user.txt`; `NODE_PASSWORD` / `auth_secret.txt`
- Default ports: 8080 (HTTP), 7878 (TCP P2P), 5000 (UDP)

### Windows Firewall Guidance

- On peers (Inbound):
```powershell
New-NetFirewallRule -DisplayName "MeshMind HTTP 8080" -Direction Inbound -Protocol TCP -LocalPort 8080 -Action Allow -Profile Private,Public
New-NetFirewallRule -DisplayName "MeshMind P2P TCP 7878" -Direction Inbound -Protocol TCP -LocalPort 7878 -Action Allow -Profile Private,Public
New-NetFirewallRule -DisplayName "MeshMind UDP 5000" -Direction Inbound -Protocol UDP -LocalPort 5000 -Action Allow -Profile Private,Public
```
- On host (Outbound for the app binary):
```powershell
$exe = "C:\\Users\\Lenovo\\Documents\\VIT\\TY\\Sem 5\\CN\\LLM-Network\\target\\release\\instance.exe"
New-NetFirewallRule -DisplayName "MeshMind instance.exe OUT 8080" -Direction Outbound -Protocol TCP -RemotePort 8080 -Action Allow -Program $exe -Profile Any
New-NetFirewallRule -DisplayName "MeshMind instance.exe OUT LAN"  -Direction Outbound -Action Allow -Program $exe -RemoteAddress 192.168.0.0/16 -Profile Any
```

## Troubleshooting

- 0 peers or empty shared files
  - Ensure peer is running: `Test-NetConnection <peer-ip> -Port 8080`
  - Host must allow outbound 8080 for `instance.exe`
  - Peer must allow inbound 8080/7878/5000
  - `p2p_secret.txt` must match on all nodes
- 401 on `/api/files`
  - Use header: `-Headers @{ "x-peer-llm" = "1" }` or login to obtain a session cookie
- Logs too fast
  - `cargo run --release 2>&1 | Tee-Object -FilePath host.log`
  - Remote listing fetches are throttled (~15s TTL)

## Evaluation Methodology and Metrics

This section outlines how to measure MeshMind performance for inclusion in an academic evaluation. Replace placeholders with your measured values and attach plots.

### 1) Latency (HTTP/P2P)

- Objective: measure end‑to‑end latency for key paths: `/api/files` aggregation, proxy file download, and chat request to LLM host.
- Procedure (example, PowerShell):
  ```powershell
  # Cold and warm measurements
  $h = @{ "x-peer-llm" = "1" }
  Measure-Command { Invoke-RestMethod -Headers $h http://localhost:8080/api/files > $null } | Select-Object TotalMilliseconds
  Measure-Command { Invoke-RestMethod -Headers $h http://localhost:8080/api/peer-file/192.168.0.108/<filename> -OutFile NUL } | Select-Object TotalMilliseconds
  ```
- Report: p50/p95/p99 latency per endpoint, cold vs warm.

### 2) Availability

- Objective: quantify success ratio under intermittent peer availability.
- Procedure:
  - Run host and N peers; periodically stop one peer; call `/api/files` every 5s for 10 minutes.
  - Record HTTP status codes and whether remote merge found K>0 items.
- Metric: availability = successful responses / total; include time‑to‑recovery after a peer returns.

### 3) Bandwidth

- Objective: estimate per‑operation bandwidth: UDP discovery overhead, TCP file propagation, proxy downloads.
- Procedure:
  - Use Windows Resource Monitor or `Get-NetAdapterStatistics` before/after operations.
  - For proxy download, note payload size vs observed bytes.
- Metric: bytes transferred per operation; overhead ratio = (total bytes / payload bytes).

### 4) CPU/Memory footprint

- Objective: quantify host and peer resource usage at idle and under load.
- Procedure: Task Manager/PerfMon sampling during 60s steady‑state; repeat for varying peer counts.
- Metric: avg and peak CPU %, working set MB.

### 5) Report Template (replace with your results)

| Metric | Scenario | p50 | p95 | p99 | Notes |
|---|---|---:|---:|---:|---|
| `/api/files` latency (ms) | 3 peers online |  |  |  |  |
| Proxy download latency (ms) | 10 MB file |  |  |  |  |
| Availability (%) | peer churn every 60s |  |  |  |  |
| Bandwidth overhead | 10 MB proxy |  |  |  |  |
| CPU / Memory | host idle/load |  |  |  |  |

Add plots for time series and CDFs. Document hardware, Wi‑Fi/LAN conditions, and versions.

## Future Scope

- Model orchestration: dynamic selection based on latency, cost, and accuracy feedback.
- Peer validation and trust scoring; rotating HMAC secrets.
- Encrypted query/result sharing; differential privacy options.
- Decentralized cache/gossip of file indexes and embeddings.
- WAN overlay support with NAT traversal.
- Auto‑update mechanism for peers (versioned binary distribution over `/api/update`).


## Highlights

- Automatic peer discovery on LAN (UDP 5000) and resilient TCP links (7878)
- Shared LLM access via the most capable reachable host (Ollama integration)
- File sharing and aggregation from all peers into a single panel
- Real‑time conversation sync (local and per‑peer histories)
- Secure peer trust via HMAC‑signed announcements (shared P2P secret)
- Same‑origin proxy to fetch peer files without exposing cross‑origin cookies
- Built with Rust (Actix Web) + TypeScript/React UI (Vite), packaged for one‑binary deploy

## Architecture

- Web/API server (Actix Web) on 0.0.0.0:8080
  - Authentication via signed cookie (HS256 JWT)
  - Internal header bypass for peer calls (`x-peer-llm: 1`) to `/api/files` and proxy route
  - Static UI embedded via rust‑embed
- P2P transport
  - UDP broadcaster/receiver (5000): periodic announcements + discovery
  - TCP connector/listener (7878): control + file propagation
- Persistence layer
  - Uploaded files (local) and received files (by peer IP)
  - Conversations: local and per‑peer (`received/<peer-ip>/local.json`)
- LLM integration
  - Ollama detection and client on the LLM host node
  - Remote usage by peers when no local LLM is present

## Network and Ports

- 8080/TCP: HTTP API + Web UI
- 7878/TCP: P2P control and file broadcast channel
- 5000/UDP: Peer discovery and announcements

Firewall guidance (Windows):

- Inbound on peers (Admin PowerShell):
  ```powershell
  New-NetFirewallRule -DisplayName "MeshMind HTTP 8080" -Direction Inbound -Protocol TCP -LocalPort 8080 -Action Allow -Profile Private,Public
  New-NetFirewallRule -DisplayName "MeshMind P2P TCP 7878" -Direction Inbound -Protocol TCP -LocalPort 7878 -Action Allow -Profile Private,Public
  New-NetFirewallRule -DisplayName "MeshMind UDP 5000" -Direction Inbound -Protocol UDP -LocalPort 5000 -Action Allow -Profile Private,Public
  ```
- Outbound on host (so the app can call peers):
  ```powershell
  $exe = "C:\\Users\\Lenovo\\Documents\\VIT\\TY\\Sem 5\\CN\\LLM-Network\\target\\release\\instance.exe"
  New-NetFirewallRule -DisplayName "MeshMind instance.exe OUT 8080" -Direction Outbound -Protocol TCP -RemotePort 8080 -Action Allow -Program $exe -Profile Any
  New-NetFirewallRule -DisplayName "MeshMind instance.exe OUT LAN"  -Direction Outbound -Action Allow -Program $exe -RemoteAddress 192.168.0.0/16 -Profile Any
  ```

## Security Model

- P2P trust: a shared HMAC secret (`P2P_HMAC_SECRET` or `p2p_secret.txt`) signs peer announcements and file meta
- HTTP auth: username/password configurable; session cookie (JWT HS256) protects `/api/*`
- Internal peer calls: requests with header `x-peer-llm: 1` are accepted for read‑only file listing and proxy download
- Same‑origin proxy: `/api/peer-file/{ip}/{filename}` fetches a peer file with the internal header and returns bytes to the browser (no CORS cookies)

## File Sharing and Aggregation

`GET /api/files` merges three sources into one list:
- Local uploads stored on this node
- Received binaries under `received/<peer-ip>/` (if present)
- Live fetch from peers’ `/api/files` (throttled), deduped by `(filename, uploader_ip)`

To download:
- Local: `GET /api/files/{filename}`
- Proxy to peer: `GET /api/peer-file/{ip}/{filename}` (requires `x-peer-llm: 1` or a session)

## Key API Endpoints

- `GET /api/status` → `{ is_llm_host, peer_count }`
- `POST /api/auth/login` → sets session cookie
- `POST /api/auth/logout`
- `GET /api/files` → aggregated file list (auth or `x-peer-llm`)
- `GET /api/files/{filename}` → local download
- `GET /api/peer-file/{ip}/{filename}` → proxy download from peer (auth or `x-peer-llm`)
- `POST /api/upload` → multipart form field `file`
- `GET /peers` → per‑peer conversation summary (auth)

## Build and Run

Prerequisites:
- Rust stable
- Node.js (only required to develop/rebuild the UI; the release binary embeds assets)
- Ollama (only on a node that will act as LLM host)

Host (LLM or regular node):
```bash
cargo build --release
cargo run --release
# UI: http://localhost:8080/app/
```

Peer (Option A: build on peer):
```bash
git clone <this repo>
cargo run --release
```

Peer (Option B: run a prebuilt binary):
1) Copy `target/release/instance.exe` and `p2p_secret.txt` (must match host) to a folder on the peer, e.g. `C:\MeshMind\`
2) Open peer inbound firewall (see above)
3) Run:
```powershell
cd C:\MeshMind
./instance.exe
```

## Configuration

- `P2P_HMAC_SECRET` env var or `p2p_secret.txt` file (same value on all nodes)
- `NODE_USERNAME` / `auth_user.txt`; `NODE_PASSWORD` / `auth_secret.txt`
- Ports are fixed by default: 8080/7878/5000 (can be changed in code)

## Troubleshooting

- Host sees 0 peers or file list never shows peer files:
  - Ensure peer is running and reachable: `Test-NetConnection <peer-ip> -Port 8080`
  - On HOST, allow outbound 8080 for `instance.exe`
  - On PEER, allow inbound 8080/7878/5000
  - Confirm `p2p_secret.txt` matches on all nodes
- 401 when calling `/api/files` from PowerShell:
  - Use header: `-Headers @{ "x-peer-llm" = "1" }`, or login first and pass the session
- Logs scroll too fast:
  - Output to file: `cargo run --release 2>&1 | Tee-Object -FilePath host.log`
  - Remote fetches are throttled internally (~15s TTL)

## Research Notes (for paper writing)

- Discovery: UDP broadcasts announce availability and collect peer IPs for subsequent HTTP/TCP use
- Trust: symmetric HMAC authenticates announcements without PKI; secret distribution is out of band
- Aggregation: a hybrid model that merges static received binaries with live peer lists to reduce inconsistency
- UX: same‑origin proxy avoids cross‑origin cookie leakage while enabling direct peer downloads
- Deployment: single Rust binary with embedded assets; peers can run from a copied exe with matching secret

## License

MIT License