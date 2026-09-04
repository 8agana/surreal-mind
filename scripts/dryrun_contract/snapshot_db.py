#!/usr/bin/env python3
"""Deterministic snapshot of the throwaway SurrealDB test namespace for the
fed-734b8f dry-run contract test (step 0.5). Queries every table the remini
task set can touch, sorts rows by id, and writes a stable JSON snapshot plus
a SHA-256 digest so before/after comparisons are exact and reproducible.
"""
import base64
import hashlib
import http.client
import json
import os

ENDPOINT_HOST = os.environ.get("SNAPSHOT_DB_HOST", "127.0.0.1")
ENDPOINT_PORT = int(os.environ.get("SNAPSHOT_DB_PORT", "8100"))
NS = os.environ.get("SNAPSHOT_DB_NS", "test_fed734b8f")
DB = os.environ.get("SNAPSHOT_DB_DB", "dry")
USER = os.environ.get("SNAPSHOT_DB_USER", "root")
PASS = os.environ.get("SNAPSHOT_DB_PASS", "root")

TABLES = [
    "thoughts",
    "kg_entities",
    "kg_edges",
    "kg_observations",
    "correction_events",
    "agent_exchanges",
    "tool_sessions",
    "kg_entity_candidates",
    "kg_edge_candidates",
    "kg_blocklist",
    "kg_boundaries",
    "recalls",
]


def query(sql: str):
    conn = http.client.HTTPConnection(ENDPOINT_HOST, ENDPOINT_PORT, timeout=15)
    auth = base64.b64encode(f"{USER}:{PASS}".encode()).decode()
    headers = {
        "Authorization": f"Basic {auth}",
        "surreal-ns": NS,
        "surreal-db": DB,
        "Accept": "application/json",
    }
    conn.request("POST", "/sql", body=sql, headers=headers)
    resp = conn.getresponse()
    body = resp.read().decode()
    conn.close()
    data = json.loads(body)
    if not data or data[0].get("status") != "OK":
        raise RuntimeError(f"query failed: {sql!r} -> {body}")
    return data[0]["result"]


def snapshot():
    snap = {}
    for t in TABLES:
        rows = query(f"SELECT * FROM {t} ORDER BY id ASC;")
        # Stringify record ids and datetimes deterministically via json default
        norm = json.loads(json.dumps(rows, default=str, sort_keys=True))
        norm_sorted = sorted(norm, key=lambda r: str(r.get("id", "")))
        snap[t] = norm_sorted
    return snap


def main():
    snap = snapshot()
    blob = json.dumps(snap, sort_keys=True, indent=2)
    digest = hashlib.sha256(blob.encode()).hexdigest()
    out = {"digest": digest, "counts": {t: len(v) for t, v in snap.items()}, "tables": snap}
    print(json.dumps(out, indent=2))


if __name__ == "__main__":
    main()
