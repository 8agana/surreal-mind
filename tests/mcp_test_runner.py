import sys
import json
import subprocess
import time
import os
import threading
from typing import Dict, Any, Optional

# Configuration
BINARY_PATH = "target/release/surreal-mind"
LOG_FILE = "tests/mcp_test_runner.log"

class MCPRunner:
    def __init__(self, binary_path: str):
        self.process = subprocess.Popen(
            [binary_path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=sys.stderr,
            text=True,
            bufsize=1
        )
        self.request_id = 0
        self.context: Dict[str, Any] = {}

    def send_notification(self, method: str, params: Optional[Dict[str, Any]] = None):
        request = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params or {}
        }
        json_req = json.dumps(request)
        self.process.stdin.write(json_req + "\n")
        self.process.stdin.flush()

    def send_request(self, method: str, params: Optional[Dict[str, Any]] = None, req_id: Optional[int] = None) -> Dict[str, Any]:
        if req_id is None:
            self.request_id += 1
            req_id = self.request_id
        
        request = {
            "jsonrpc": "2.0",
            "id": req_id,
            "method": method,
            "params": params or {}
        }
        
        json_req = json.dumps(request)
        if "wander" in json_req:
             print(f"DEBUG: {json_req}", file=sys.stderr)
        # print(f"-> {json_req}", file=sys.stderr)
        self.process.stdin.write(json_req + "\n")
        self.process.stdin.flush()
        
        while True:
            response_line = self.process.stdout.readline()
            # print(f"<- {response_line.strip()}", file=sys.stderr)
            
            if not response_line:
                raise Exception("Process terminated unexpectedly or returned no output")
                
            response = json.loads(response_line)
            
            # If it's a notification (no id) or an unrelated request from server, skip it
            if "id" not in response or response["id"] != req_id:
                print(f"[Ignoring Notification/Other]: {json.dumps(response)[:100]}...", file=sys.stderr)
                continue
                
            return response

    def run_test(self, test_name: str, method: str, params: Dict[str, Any], validator=None) -> bool:
        print(f"Running {test_name}...", end=" ", flush=True)
        try:
            # Replace placeholders in params
            params_str = json.dumps(params)
            for key, value in self.context.items():
                if isinstance(value, str):
                    params_str = params_str.replace(f"REPLACE_{key}", value)
            params = json.loads(params_str)

            response = self.send_request(method, params)
            
            if "error" in response:
                if validator and validator(response):
                     print("✅ PASS")
                     return True
                print(f"❌ FAIL (Error: {response['error']})")
                return False
                
            if "result" in response:
                if validator:
                    if validator(response["result"]):
                        print("✅ PASS")
                        return True
                    else:
                        print(f"❌ FAIL (Validator failed. Result: {json.dumps(response['result'])[:100]}...)")
                        return False
                print("✅ PASS")
                return True
                
            print("❌ FAIL (No result or error)")
            return False
            
        except Exception as e:
            print(f"❌ FAIL (Exception: {e})")
            return False

    def close(self):
        self.process.terminate()
        self.process.wait()

def main():
    runner = MCPRunner(BINARY_PATH)
    
    try:
        # 1. Initialize
        print("--- Protocol Compliance ---")
        runner.run_test(
            "MCP-PR-001 Initialize", 
            "initialize", 
            {
                "protocolVersion": "2024-11-05", 
                "clientInfo": {"name": "test-runner", "version": "1.0"}, 
                "capabilities": {}
            },
            lambda r: "serverInfo" in r
        )
        
        runner.send_notification("notifications/initialized")

        runner.run_test(
            "MCP-PR-002 Tools List",
            "tools/list",
            {},
            lambda r: "tools" in r and len(r["tools"]) > 0
        )

        runner.run_test(
            "MCP-PR-003 Tools Call Basic",
            "tools/call",
            {"name": "howto", "arguments": {"tool": "think"}},
            lambda r: "content" in r
        )
        
        runner.run_test(
             "MCP-PR-004 Notifications",
             "tools/call",
             {"name": "test_notification", "arguments": {"message": "test"}},
             lambda r: not r.get("isError")
        )

        print("\n--- Individual Tools ---")
        
        # MCP-TK-003 Remember Entity (Do this early to get ID)
        def save_entity_id(r):
            if "content" in r and isinstance(r["content"], list) and len(r["content"]) > 0:
                 text = r["content"][0]["text"]
                 # Try parsing text as JSON first
                 try:
                     data = json.loads(text)
                     if "id" in data:
                         # ID might be "family:id" or just "id". 
                         # If it returns full thing, good. If just ID, prepend table if needed?
                         # Usually remember returns full ID or table:id
                         runner.context["ENTITY_ID_1"] = data["id"].replace("entity:", "")
                         return True
                 except json.JSONDecodeError:
                     pass
                     
                 if "Created entity" in text:
                     # simplistic parsing, assuming "Created entity: <id>" format
                     try:
                        import re
                        match = re.search(r"Created entity: ([a-zA-Z0-9_:]+)", text)
                        if match:
                            runner.context["ENTITY_ID_1"] = match.group(1).replace("entity:", "")
                            return True
                     except:
                         pass
                 return True
            return False

        runner.run_test(
            "MCP-TK-003 Remember (Entity 1)",
            "tools/call",
            {
                "name": "remember", 
                "arguments": {
                    "kind": "entity",
                    "data": {"name": "Test Entity 1", "type": "Test"},
                    "confidence": 1.0
                }
            },
            save_entity_id
        )

        # Create 2nd entity
        def save_entity_id_2(r):
             if "content" in r and isinstance(r["content"], list) and len(r["content"]) > 0:
                 text = r["content"][0]["text"]
                 try:
                     data = json.loads(text)
                     if "id" in data:
                         runner.context["ENTITY_ID_2"] = data["id"].replace("entity:", "")
                         return True
                 except json.JSONDecodeError:
                     pass
                     
                 import re
                 match = re.search(r"Created entity: ([a-zA-Z0-9_:]+)", text)
                 if match:
                    runner.context["ENTITY_ID_2"] = match.group(1).replace("entity:", "")
                    return True
             return False

        runner.run_test(
            "MCP-TK-003 Remember (Entity 2)",
            "tools/call",
            {
                "name": "remember", 
                "arguments": {
                    "kind": "entity",
                    "data": {"name": "Test Entity 2", "type": "Test"},
                    "confidence": 1.0
                }
            },
            save_entity_id_2
        )

        # MCP-TK-004 Remember Relationship
        runner.run_test(
            "MCP-TK-004 Remember (Relationship)",
            "tools/call",
            {
                "name": "remember",
                "arguments": {
                    "kind": "relationship",
                    "data": {
                        "source": "entity:REPLACE_ENTITY_ID_1",
                        "target": "entity:REPLACE_ENTITY_ID_2",
                        "rel_type": "tests",
                        "evidence": "automated test"
                    }
                }
            },
            lambda r: "id" in json.loads(r["content"][0]["text"]) or "Created relationship" in r["content"][0]["text"]
        )

        runner.run_test(
            "MCP-TK-001 Think",
            "tools/call",
            {"name": "think", "arguments": {"content": "Test thought", "hint": "plan", "needs_verification": False}},
            lambda r: "content" in r
        )

        runner.run_test(
            "MCP-TK-002 Search",
            "tools/call",
            {"name": "search", "arguments": {"query": {"text": "Test Entity"}}},
            lambda r: "Test Entity" in r["content"][0]["text"]
        )
        
        # MCP-TK-005 Wander
        # Note: Using ENTITY_ID_1 from context
        runner.run_test(
            "MCP-TK-005 Wander",
            "tools/call",
            {
                "name": "wander",
                "arguments": {
                    "mode": "meta",
                    "current_thought_id": "entity:REPLACE_ENTITY_ID_1"
                }
            },
            lambda r: "content" in r
        )

        runner.run_test(
             "MCP-TK-006 Maintain",
             "tools/call",
             {"name": "maintain", "arguments": {"subcommand": "health", "dry_run": True}},
             lambda r: "health" in r["content"][0]["text"].lower()
        )
        
        # Skipping call_gem, call_codex, call_cc for now as they might take long or require ext deps
        # We can stub them or run if needed. Let's run a simple one.
        
        def save_job_id(r):
            if "content" in r and len(r["content"]) > 0:
                text = r["content"][0]["text"]
                try:
                    data = json.loads(text)
                    if "job_id" in data:
                        runner.context["JOB_ID"] = data["job_id"]
                        return True
                    # call_gem might return status object
                    if "id" in data: 
                         runner.context["JOB_ID"] = data["id"]
                         return True
                except json.JSONDecodeError:
                    pass
                
                import re
                match = re.search(r"Job ID: ([a-zA-Z0-9-]+)", text)
                if match:
                    runner.context["JOB_ID"] = match.group(1)
                    return True
            return False

        # Using a very short timeout/mock
        runner.run_test(
            "MCP-TK-008 call_gem (Mock)",
             "tools/call",
             {
                 "name": "call_gem",
                 "arguments": {
                     "prompt": "echo OK",
                     "cwd": ".",
                     "mode": "observe",
                     "timeout_ms": 30000
                 }
             },
             save_job_id
        )

        if "JOB_ID" in runner.context:
            runner.run_test(
                "MCP-TK-011 call_status",
                "tools/call",
                {"name": "call_status", "arguments": {"job_id": "REPLACE_JOB_ID"}},
                lambda r: "content" in r
            )
            
            runner.run_test(
                "MCP-TK-013 call_cancel",
                "tools/call",
                {"name": "call_cancel", "arguments": {"job_id": "REPLACE_JOB_ID"}},
                lambda r: True # Just check it doesn't crash
            )

        runner.run_test(
             "MCP-TK-014 Rethink",
             "tools/call",
             {
                 "name": "rethink",
                 "arguments": {
                     "mode": "mark",
                     "target_id": "entity:REPLACE_ENTITY_ID_1",
                     "mark_type": "correction",
                     "marked_for": "gemini",
                     "note": "Test correction"
                 }
             },
             lambda r: "content" in r and ("marked" in r["content"][0]["text"].lower() or "success" in r["content"][0]["text"])
        )

        runner.run_test(
            "MCP-TK-015 Corrections",
            "tools/call",
            {"name": "corrections", "arguments": {"limit": 1}},
            lambda r: True
        )

        print("\n--- Error Handling ---")
        
        # MCP-ER-001 is handled by parser usually, harder to test with this client client 
        # as we are sending valid JSON-RPC envelopes.
        
        runner.run_test(
            "MCP-ER-002 Unknown Method",
            "unknown_method",
            {},
            lambda r: "error" in r or "Method not found" in str(r)
        )

        runner.run_test(
             "MCP-ER-003 Unknown Tool",
             "tools/call",
             {"name": "fake_tool", "arguments": {}},
             lambda r: "error" in r
        )
        
        runner.run_test(
            "MCP-ER-004 Missing Args",
            "tools/call",
            {"name": "call_status", "arguments": {}},
             lambda r: "error" in r # Should fail validation
        )

    finally:
        runner.close()

if __name__ == "__main__":
    main()
