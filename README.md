# Chakravyuh-1.0

A distributed peer-to-peer LLM network application that enables sharing of LLM capabilities across nodes.

## Features

- Peer-to-peer network communication using TCP and UDP
- Automatic peer discovery via UDP broadcasts
- Shared LLM (Language Model) access across peers
- Real-time conversation synchronization
- Persistent conversation storage
- Web-based user interface

## Requirements

- Rust (latest stable version)
- Node.js (for web interface)
- Ollama (for LLM capabilities)

## Setup

1. Clone the repository:
```bash
git clone https://github.com/Omkar2k5/Chakravyuh-1.0.git
cd Chakravyuh-1.0
```

2. Build and run the project:
```bash
cargo build --release
cargo run --release
```

3. For LLM host setup, run the provided batch script:
```bash
ollama_host.bat
```

## Architecture

- TCP module: Handles peer connections and message passing
- UDP module: Manages peer discovery and network broadcasts
- LLM module: Interfaces with Ollama for language model capabilities
- Web Interface: React-based UI for user interaction

## License

MIT License 