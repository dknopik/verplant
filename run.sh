#!/bin/bash

# Verplant - Subway Game Development Script

echo "🚇 Building Verplant Subway Game..."

# Build the server
echo "📡 Building server..."
cargo build -p verplant_server

if [ $? -ne 0 ]; then
    echo "❌ Server build failed"
    exit 1
fi

echo "✅ Server built successfully"

# Build the WASM client
echo "🌐 Building WASM client..."
cd client

# Check if wasm-pack is installed
if ! command -v wasm-pack &> /dev/null; then
    echo "⚠️  wasm-pack not found. Installing..."
    exit 1
    #curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

wasm-pack build --target web --out-dir pkg

if [ $? -ne 0 ]; then
    echo "❌ WASM client build failed"
    exit 1
fi

echo "✅ WASM client built successfully"

cd ..

echo ""
echo "🎮 Verplant is ready to play!"
echo ""
echo "To start the game:"
echo "1. Terminal 1: cargo run -p verplant_server"
echo "2. Terminal 2: cd client && python -m http.server 8000"
echo "3. Open http://localhost:8000 in your browser"
echo ""
echo "Have fun playing the subway game! 🚇"