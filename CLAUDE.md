# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Architecture

This is a Rust workspace project with a client-server architecture:

- **Root workspace**: Manages the entire project with workspace dependencies
- **client/**: Client application (`verplant_client`) - currently minimal with empty main.rs
- **server/**: Server application (`verplant_server`) - basic "Hello, world!" implementation  
- **shared/**: Shared library (`verplant`) for common code between client and server - currently empty

All components use Rust edition 2024 and are at version 0.1.0, indicating this is an early-stage project.

## Development Commands

### Building
```bash
# Build entire workspace
cargo build

# Build specific component
cargo build -p verplant_client
cargo build -p verplant_server
cargo build -p verplant

# Release build
cargo build --release
```

### Running
```bash
# Run server
cargo run -p verplant_server

# Run client  
cargo run -p verplant_client
```

### Testing
```bash
# Run all tests
cargo test

# Test specific package
cargo test -p verplant_server
```

### Building WASM client
```bash
# Install wasm-pack (if not already installed)
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# Build WASM client
cd client
wasm-pack build --target web --out-dir pkg

# Serve the client (requires a local HTTP server)
# You can use Python: python -m http.server 8000
# Or Node.js: npx serve .
# Then visit http://localhost:8000
```

### Development workflow
```bash
# Terminal 1: Run the server
cargo run -p verplant_server

# Terminal 2: Build and serve client
cd client
wasm-pack build --target web --out-dir pkg
python -m http.server 8000

# Visit http://localhost:8000 in browser
```

### Other useful commands
```bash
# Check code without building
cargo check

# Format code
cargo fmt

# Run clippy linter
cargo clippy
```