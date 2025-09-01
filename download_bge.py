#!/usr/bin/env python3
"""Download BGE-small-en-v1.5 model files for Candle"""

from huggingface_hub import snapshot_download
import os

model_id = "BAAI/bge-small-en-v1.5"
local_dir = "./models/bge-small-en-v1.5"

print(f"Downloading {model_id} to {local_dir}...")

# Download the model files
snapshot_download(
    repo_id=model_id,
    local_dir=local_dir,
    local_dir_use_symlinks=False,
    ignore_patterns=["*.h5", "*.ot", "*.msgpack", "flax_model*", "rust_model*"]
)

print(f"Model downloaded to {local_dir}")
print("\nFiles downloaded:")
for root, dirs, files in os.walk(local_dir):
    for file in files:
        filepath = os.path.join(root, file)
        size = os.path.getsize(filepath) / (1024 * 1024)  # MB
        print(f"  {filepath}: {size:.2f} MB")