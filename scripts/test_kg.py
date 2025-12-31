import urllib.request
import json
import subprocess
import base64
import sys

URL = "http://127.0.0.1:8000/sql"
NS = "surreal_mind"
DB = "consciousness"
USER = "root"
PASS = "root"

def query(sql):
    req = urllib.request.Request(URL, data=sql.encode('utf-8'))
    req.add_header("Accept", "application/json")
    req.add_header("NS", NS)
    req.add_header("DB", DB)
    
    # Basic Auth
    auth_str = f"{USER}:{PASS}"
    auth_b64 = base64.b64encode(auth_str.encode('utf-8')).decode('utf-8')
    req.add_header("Authorization", f"Basic {auth_b64}")
    
    with urllib.request.urlopen(req) as response:
        return json.load(response)

print("Checking pending thoughts...")
try:
    res = query("SELECT count() FROM thoughts WHERE extracted_to_kg = false OR extracted_to_kg = NONE")
    # res is list of results
    if res and 'result' in res[0]:
         # [{'result': [{'count': 0}], ...}]
         # Note: count() returns array of objects like [{count: N}]
         count = res[0]['result'][0]['count']
         print(f"Pending thoughts: {count}")
         
         if count == 0:
             print("Inserting test thought...")
             query("CREATE thoughts SET content = 'Rust is a systems programming language that runs blazingly fast. It prevents memory errors and guarantees thread safety.', extracted_to_kg = false, created_at = time::now()")
             print("Inserted.")
    else:
        print(f"Unexpected response format: {res}")

except Exception as e:
    print(f"Error checking/inserting: {e}")

print("Running kg_populate binary...")
# We use check=False to allow it to fail without crashing the script, capturing output
subprocess.run(["cargo", "run", "--bin", "kg_populate"], cwd="/Users/samuelatagana/Projects/LegacyMind/surreal-mind")
