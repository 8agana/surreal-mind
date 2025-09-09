#!/usr/bin/env python3
import os, json, base64
from urllib import request
from typing import Dict, Any, Tuple
from pathlib import Path

SURREAL_URL = os.environ.get("SURR_HTTP", "http://127.0.0.1:8000/sql")
ENV_PATH = "/Users/samuelatagana/Projects/LegacyMind/.env"

CANDIDATE_USER_KEYS = ["SURR_USER", "SURREAL_USER", "SURREALDB_USER", "DB_USER", "USER"]
CANDIDATE_PASS_KEYS = ["SURR_PASS", "SURREAL_PASS", "SURREALDB_PASS", "DB_PASS", "PASS", "PASSWORD"]

def load_env_credentials(env_path: str) -> Tuple[str, str]:
    u = os.environ.get("SURR_USER"); p = os.environ.get("SURR_PASS")
    if u and p: return (u, p)
    creds: Dict[str,str] = {}
    if Path(env_path).exists():
        for line in Path(env_path).read_text().splitlines():
            line = line.strip()
            if not line or line.startswith('#') or '=' not in line: continue
            k,v = line.split('=',1)
            creds[k.strip()] = v.strip().strip('"').strip("'")
    user = next((creds[k] for k in CANDIDATE_USER_KEYS if k in creds and creds[k]), None)
    pwd  = next((creds[k] for k in CANDIDATE_PASS_KEYS if k in creds and creds[k]), None)
    if user and pwd: return (user, pwd)
    if user and not pwd: return (user, user)
    raise RuntimeError("No SurrealDB creds found")

from urllib import error as urlerror

def http_sql(ns: str, db: str, sql: str, auth_b64: str) -> Any:
    req = request.Request(SURREAL_URL, data=sql.encode('utf-8'), method='POST')
    req.add_header('Content-Type','text/plain')
    req.add_header('Accept','application/json')
    req.add_header('NS', ns)
    req.add_header('DB', db)
    req.add_header('Authorization', f'Basic {auth_b64}')
    try:
        with request.urlopen(req, timeout=60) as resp:
            body = resp.read().decode('utf-8')
            return json.loads(body) if body else None
    except urlerror.HTTPError as e:
        body = e.read().decode('utf-8', errors='ignore')
        return json.loads(body) if body else {"status":"http_error","code":e.code}


def _extract_first_value(res: Any, key: str) -> Any:
    try:
        if isinstance(res, list) and res and 'result' in res[0] and res[0]['result']:
            row = res[0]['result'][0]
            return row.get(key)
    except Exception:
        return None
    return None


def verify(ns: str, db: str, label: str, uniform: str, auth_b64: str) -> Dict[str,Any]:
    thoughts = _extract_first_value(http_sql(ns, db, f"SELECT count() AS thoughts FROM thought WHERE uniform='{uniform}' AND embed_origin='ui2'", auth_b64), 'thoughts')
    entities = _extract_first_value(http_sql(ns, db, f"SELECT count() AS entities FROM entity WHERE uniform='{uniform}' AND embed_origin='ui2'", auth_b64), 'entities')
    observations = _extract_first_value(http_sql(ns, db, f"SELECT count() AS observations FROM observation WHERE uniform='{uniform}' AND embed_origin='ui2'", auth_b64), 'observations')
    edges = _extract_first_value(http_sql(ns, db, f"SELECT count() AS edges FROM relates_to WHERE migrated_from='ui2'", auth_b64), 'edges')
    ts = http_sql(ns, db, f"SELECT min(created_at) AS min_ts, max(created_at) AS max_ts FROM thought WHERE uniform='{uniform}' AND embed_origin='ui2'", auth_b64)
    min_ts = _extract_first_value(ts, 'min_ts')
    max_ts = _extract_first_value(ts, 'max_ts')
    return {
        'label': label,
        'ns': ns,
        'db': db,
        'uniform': uniform,
        'thoughts': thoughts,
        'entities': entities,
        'observations': observations,
        'edges': edges,
        'min_ts': min_ts,
        'max_ts': max_ts,
    }


def main():
    user, pwd = load_env_credentials(ENV_PATH)
    auth_b64 = base64.b64encode(f"{user}:{pwd}".encode()).decode()

    lm = verify(ns='surreal_mind', db='conciousness', label='LegacyMind', uniform='LegacyMind', auth_b64=auth_b64)
    ph = verify(ns='photography', db='work', label='Photography', uniform='Photography', auth_b64=auth_b64)

    print(json.dumps(lm, ensure_ascii=False))
    print(json.dumps(ph, ensure_ascii=False))

if __name__ == '__main__':
    main()
