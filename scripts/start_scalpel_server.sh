#!/bin/bash

# start_scalpel_server.sh
# Launches mistralrs-server with the Hermes 3 (Llama 3.2 3B) model for Scalpel.

PORT=8111
MODEL_REPO="NousResearch/Hermes-3-Llama-3.2-3B-GGUF"
MODEL_FILE="Hermes-3-Llama-3.2-3B.Q4_K_M.gguf"

echo "Starting Scalpel Backend (Hermes 3)..."
echo "Model: $MODEL_REPO ($MODEL_FILE)"
echo "Port: $PORT"
echo "Detailed logs: stderr"

# Check if mistralrs-server is in the path
if ! command -v mistralrs-server &> /dev/null; then
    echo "Error: mistralrs-server not found. Is it in your PATH?"
    echo "Try: export PATH=\$PATH:~/.cargo/bin"
    exit 1
fi

nohup mistralrs-server --port $PORT gguf -m $MODEL_REPO -f $MODEL_FILE > scalpel_server.log 2>&1 &
PID=$!

echo "Server started in background with PID: $PID"
echo "Logs are being written to: scalpel_server.log"
echo "To stop the server, run: kill $PID"
