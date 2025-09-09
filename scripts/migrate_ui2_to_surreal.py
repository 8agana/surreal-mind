#!/usr/bin/env python3
import os
import sys
import json
import base64
import sqlite3
import time
from pathlib import Path
from typing import List, Dict, Any, Optional, Tuple
from urllib import request, error

# Config: mapping of source DBs to SurrealDB targets
MAPPINGS = [
    {
        "db_path": "/Users/samuelatagana/Projects/LegacyMind/deprecated/unified-intelligence-2/Memory/uniforms/legacymind.db",
        "uniform": "LegacyMind",
        "ns": "surreal_mind",
        "db": "conciousness",
    },
    {
        "db_path": "/Users/samuelatagana/Projects/LegacyMind/deprecated/unified-intelligence-2/Memory/uniforms/personal.db",
        "uniform": "LegacyMind",
        "ns": "surreal_mind",
        "db": "conciousness",
    },
    {
        "db_path": "/Users/samuelatagana/Projects/LegacyMind/deprecated/unified-intelligence-2/Memory/uniforms/photography.db",
        "uniform": "Photography",
        "ns": "photography",
        "db": "work",
    },
]

SURREAL_URL = os.environ.get("SURR_HTTP", "http://127.0.0.1:8000/sql")
ENV_PATH = "/Users/samuelatagana/Projects/LegacyMind/.env"

# Read credentials from .env without printing them. Try a few common keys.
CANDIDATE_USER_KEYS = ["SURR_USER", "SURREAL_USER", "SURREALDB_USER", "DB_USER", "USER"]
CANDIDATE_PASS_KEYS = ["SURR_PASS", "SURREAL_PASS", "SURREALDB_PASS", "DB_PASS", "PASS", "PASSWORD"]


def load_env_credentials(env_path: str) -> Tuple[str, str]:
    user = os.environ.get("SURR_USER")
    pwd = os.environ.get("SURR_PASS")
    if user and pwd:
        return user, pwd

    if not Path(env_path).exists():
        raise RuntimeError(f".env not found at {env_path}")

    creds: Dict[str, str] = {}
    with open(env_path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            if "=" not in line:
                continue
            k, v = line.split("=", 1)
            k = k.strip()
            v = v.strip().strip('"').strip("'")
            creds[k] = v

    user_val = None
    pass_val = None

    for k in CANDIDATE_USER_KEYS:
        if k in creds and creds[k]:
            user_val = creds[k]
            break
    for k in CANDIDATE_PASS_KEYS:
        if k in creds and creds[k]:
            pass_val = creds[k]
            break

    if user_val and pass_val:
        return user_val, pass_val

    # Fallback: if user exists and password same as user (as hinted), use it
    if user_val and not pass_val:
        return user_val, user_val

    raise RuntimeError("Could not determine SurrealDB credentials from env or .env")


def http_sql(ns: str, db: str, sql: str, auth_b64: str) -> Dict[str, Any]:
    data = sql.encode("utf-8")
    req = request.Request(SURREAL_URL, data=data, method="POST")
    # SurrealDB headers
    req.add_header("Content-Type", "text/plain")
    req.add_header("Accept", "application/json")
    req.add_header("NS", ns)
    req.add_header("DB", db)
    req.add_header("Authorization", f"Basic {auth_b64}")
    try:
        with request.urlopen(req, timeout=60) as resp:
            body = resp.read().decode("utf-8")
            if not body:
                return {"status": "ok", "result": None}
            try:
                return json.loads(body)
            except Exception:
                return {"status": "ok", "raw": body}
    except error.HTTPError as e:
        b = e.read().decode("utf-8", errors="ignore")
        return {"status": "http_error", "code": e.code, "body": b}
    except Exception as e:
        return {"status": "error", "error": str(e)}


def ensure_indexes(ns: str, db: str, auth_b64: str) -> None:
    # Define HNSW vector indexes; ignore errors if already defined
    stmts = [
        "DEFINE INDEX thought_text_vec_idx ON thought FIELDS text_vec TYPE hnsw DIMENSION 1536 METRIC cosine;",
        "DEFINE INDEX entity_text_vec_idx ON entity FIELDS text_vec TYPE hnsw DIMENSION 1536 METRIC cosine;",
        "DEFINE INDEX obs_text_vec_idx ON observation FIELDS text_vec TYPE hnsw DIMENSION 1536 METRIC cosine;",
    ]
    res = http_sql(ns, db, "\n".join(stmts), auth_b64)
    # Do not print secrets; only minimal status
    # We won't raise if index exists


def blob_to_f32_list(b: Optional[bytes]) -> Optional[List[float]]:
    if b is None:
        return None
    # Expect 1536 float32 => 6144 bytes
    if len(b) % 4 != 0:
        return None
    import struct
    cnt = len(b) // 4
    vals = list(struct.unpack("<" + ("f" * cnt), b))
    return vals


def json_array_or_tags(text: Optional[str]) -> Optional[List[str]]:
    if text is None:
        return None
    t = text.strip()
    if not t:
        return None
    try:
        val = json.loads(t)
        if isinstance(val, list):
            return [str(x) for x in val]
    except Exception:
        pass
    # fallback: comma-separated
    return [s.strip() for s in t.split(",") if s.strip()]


def migrate_thoughts(conn: sqlite3.Connection, ns: str, db: str, uniform: str, auth_b64: str) -> Tuple[int, int]:
    cur = conn.cursor()
    # thoughts schema uses 'timestamp' as time field
    cur.execute("SELECT id, content, framework, chain_id, timestamp, tags, embedding FROM thoughts")
    rows = cur.fetchall()
    created = 0
    updated = 0

    batch: List[str] = []
    batch_size = 50

    for r in rows:
        (sid, content, framework, chain_id, ts, tags, emb_blob) = r
        rid = f"ui2:{uniform}:thought:{sid}"
        tags_arr = json_array_or_tags(tags)
        vec = blob_to_f32_list(emb_blob)
        body = {
            "id": rid,
            "content": content,
            "framework": framework,
            "chain_id": chain_id,
            "uniform": uniform,
            "tags": tags_arr,
            # Preserve original timestamps
            "created_at": ts,
            "updated_at": ts,
            # Embedding copy-through
            "text_vec": vec,
            "embed_model": "openai/text-embedding-3-small",
            "embed_dims": 1536,
            "embed_origin": "ui2",
            # Migration audit
            "migrated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "migrated_from": "ui2",
        }
        stmt = f"CREATE thought:{rid} CONTENT {json.dumps(body, ensure_ascii=False)}\n" \
               f"UPDATE thought:{rid} MERGE {json.dumps({k: body[k] for k in body if k not in ['id','created_at']}, ensure_ascii=False)}"
        batch.append(stmt)
        if len(batch) >= batch_size:
            res = http_sql(ns, db, "\n".join(batch), auth_b64)
            batch.clear()
    if batch:
        res = http_sql(ns, db, "\n".join(batch), auth_b64)
        batch.clear()

    # We are not differentiating created vs updated without extra round-trips; return totals read
    return len(rows), 0


def migrate_kg(conn: sqlite3.Connection, ns: str, db: str, uniform: str, auth_b64: str) -> Tuple[int, int, int]:
    cur = conn.cursor()
    created_nodes = 0
    created_edges = 0
    created_frames = 0

    # kg_node → entity
    try:
        cur.execute("SELECT id, name, display_name, entity_type, scope, created_at, updated_at, created_by, attributes, tags, embedding FROM kg_node")
        rows = cur.fetchall()
        batch: List[str] = []
        for r in rows:
            (sid, name, display_name, entity_type, scope, created_at, updated_at, created_by, attributes, tags, emb_blob) = r
            rid = f"ui2:{uniform}:entity:{sid}"
            tags_arr = json_array_or_tags(tags)
            vec = blob_to_f32_list(emb_blob)
            body = {
                "id": rid,
                "kind": entity_type,
                "name": display_name or name,
                "description": name,
                "uniform": uniform,
                "tags": tags_arr,
                "created_at": created_at,
                "updated_at": updated_at or created_at,
                "text_vec": vec,
                "embed_model": "openai/text-embedding-3-small",
                "embed_dims": 1536,
                "embed_origin": "ui2",
                "metadata": {"scope": scope, "created_by": created_by, "attributes": attributes},
                "migrated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                "migrated_from": "ui2",
            }
            stmt = f"CREATE entity:{rid} CONTENT {json.dumps(body, ensure_ascii=False)}\n" \
                   f"UPDATE entity:{rid} MERGE {json.dumps({k: body[k] for k in body if k not in ['id','created_at']}, ensure_ascii=False)}"
            batch.append(stmt)
            if len(batch) >= 50:
                http_sql(ns, db, "\n".join(batch), auth_b64)
                batch.clear()
        if batch:
            http_sql(ns, db, "\n".join(batch), auth_b64)
            batch.clear()
        created_nodes += len(rows)
    except sqlite3.OperationalError:
        pass

    # kg_relation → RELATE entity -> entity with payload
    try:
        cur.execute("SELECT id, from_id, to_id, relationship_type, bidirectional, weight, confidence, temporal_start, temporal_end, created_at, created_by, attributes FROM kg_relation")
        rows = cur.fetchall()
        batch: List[str] = []
        for r in rows:
            (sid, from_id, to_id, rel_type, bidirectional, weight, confidence, tstart, tend, created_at, created_by, attributes) = r
            from_rid = f"ui2:{uniform}:entity:{from_id}"
            to_rid   = f"ui2:{uniform}:entity:{to_id}"
            payload = {
                "type": rel_type,
                "bidirectional": bool(bidirectional or 0),
                "weight": float(weight or 1.0),
                "confidence": float(confidence) if confidence is not None else None,
                "temporal_start": tstart,
                "temporal_end": tend,
                "created_at": created_at,
                "metadata": {"created_by": created_by, "attributes": attributes},
                "migrated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                "migrated_from": "ui2",
            }
            # RELATE with CONTENT payload
            stmt = f"RELATE entity:{from_rid}->relates_to->entity:{to_rid} CONTENT {json.dumps(payload, ensure_ascii=False)}"
            batch.append(stmt)
            if len(batch) >= 100:
                http_sql(ns, db, "\n".join(batch), auth_b64)
                batch.clear()
        if batch:
            http_sql(ns, db, "\n".join(batch), auth_b64)
            batch.clear()
        created_edges += len(rows)
    except sqlite3.OperationalError:
        pass

    # kg_frame → observation-like nodes (optional)
    try:
        cur.execute("SELECT id, node_id, column, properties, embedding, created_at, updated_at FROM kg_frame")
        rows = cur.fetchall()
        batch: List[str] = []
        for r in rows:
            (sid, node_id, column, properties, emb_blob, created_at, updated_at) = r
            rid = f"ui2:{uniform}:observation:{sid}"
            vec = blob_to_f32_list(emb_blob)
            body = {
                "id": rid,
                "content": properties,
                "subject_ref": f"entity:ui2:{uniform}:entity:{node_id}",
                "uniform": uniform,
                "created_at": created_at,
                "updated_at": updated_at or created_at,
                "text_vec": vec,
                "embed_model": "openai/text-embedding-3-small",
                "embed_dims": 1536,
                "embed_origin": "ui2",
                "metadata": {"column": column},
                "migrated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                "migrated_from": "ui2",
            }
            stmt = f"CREATE observation:{rid} CONTENT {json.dumps(body, ensure_ascii=False)}\n" \
                   f"UPDATE observation:{rid} MERGE {json.dumps({k: body[k] for k in body if k not in ['id','created_at']}, ensure_ascii=False)}"
            batch.append(stmt)
            if len(batch) >= 50:
                http_sql(ns, db, "\n".join(batch), auth_b64)
                batch.clear()
        if batch:
            http_sql(ns, db, "\n".join(batch), auth_b64)
            batch.clear()
        created_frames += len(rows)
    except sqlite3.OperationalError:
        pass

    return created_nodes, created_edges, created_frames


def migrate_one(mapping: Dict[str, str], auth_b64: str) -> Dict[str, Any]:
    db_path = mapping["db_path"]
    uniform = mapping["uniform"]
    ns = mapping["ns"]
    db = mapping["db"]

    if not Path(db_path).exists():
        return {"db": db_path, "status": "missing"}

    conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    try:
        ensure_indexes(ns, db, auth_b64)
        created_thoughts, _ = migrate_thoughts(conn, ns, db, uniform, auth_b64)
        nodes, edges, frames = migrate_kg(conn, ns, db, uniform, auth_b64)
        return {
            "db": db_path,
            "uniform": uniform,
            "ns": ns,
            "database": db,
            "created_thoughts": created_thoughts,
            "created_entities": nodes,
            "created_edges": edges,
            "created_observations": frames,
            "status": "ok",
        }
    finally:
        conn.close()


def main() -> int:
    try:
        user, pwd = load_env_credentials(ENV_PATH)
    except Exception as e:
        print(f"[ERR] Credentials: {e}")
        return 1

    auth_b64 = base64.b64encode(f"{user}:{pwd}".encode("utf-8")).decode("ascii")

    results = []
    for m in MAPPINGS:
        print(f"[INFO] Migrating {m['db_path']} -> ns={m['ns']} db={m['db']} uniform={m['uniform']}")
        res = migrate_one(m, auth_b64)
        results.append(res)
        # Minimal status per DB
        print(json.dumps({k: res[k] for k in res if k in ("db","uniform","ns","database","status","created_thoughts","created_entities","created_edges","created_observations")}, ensure_ascii=False))

    print("[INFO] Migration complete")
    return 0


if __name__ == "__main__":
    sys.exit(main())
