#!/usr/bin/env python3
"""Experimental subscription-backed KG decision runner. No KG writes here."""
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time

AGENT = "kg-decision"
MAX_BYTES = 2 * 1024 * 1024
SCHEMA = {
    "type": "object", "additionalProperties": False,
    "properties": {
        "action": {"type": "string", "enum": ["wander", "connect", "create_entity", "observe"]},
        "parameters": {"type": "object"}, "rationale": {"type": "string"}},
    "required": ["action", "parameters", "rationale"]}


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON key")
        result[key] = value
    return result


def decode(raw):
    return json.loads(raw, object_pairs_hook=unique_object,
                      parse_constant=lambda _: (_ for _ in ()).throw(ValueError("nonfinite JSON")))


def validate(value):
    if not isinstance(value, dict) or set(value) != {"action", "parameters", "rationale"}:
        raise ValueError("invalid decision fields")
    action, params, reason = value["action"], value["parameters"], value["rationale"]
    shapes = {"wander": ({"mode"}, {"mode"}),
              "connect": ({"target", "rel_type"}, {"target", "rel_type"}),
              "create_entity": ({"name", "entity_type"}, {"name", "entity_type"}),
              "observe": ({"name", "content"}, {"name", "content"})}
    if not isinstance(action, str) or action not in shapes or not isinstance(params, dict):
        raise ValueError("invalid action or parameters")
    allowed, required = shapes[action]
    if not required <= set(params) or not set(params) <= allowed:
        raise ValueError("invalid action-specific parameter keys")
    if not isinstance(reason, str) or not reason.strip() or len(reason) > 4096:
        raise ValueError("invalid rationale")
    for text in params.values():
        if not isinstance(text, str) or not text.strip() or len(text) > 4096:
            raise ValueError("invalid parameter value")
    if action == "wander" and params["mode"] not in {"random", "semantic", "meta"}:
        raise ValueError("invalid wander mode")
    return value


def parse_stream(raw):
    if len(raw) > MAX_BYTES:
        raise ValueError("output too large")
    initialized = False
    completed = False
    decision = None
    conversation = None
    for line in raw.splitlines():
        if not line.strip():
            continue
        event = decode(line)
        if not isinstance(event, dict) or completed:
            raise ValueError("invalid event sequence")
        kind = event.get("event")
        if kind == "init":
            if initialized or event.get("init", {}).get("agent") != AGENT:
                raise ValueError("unexpected agent initialization")
            conversation = event.get("conversation_id")
            if not isinstance(conversation, str) or not conversation:
                raise ValueError("missing conversation identity")
            initialized = True
        elif kind == "step_update":
            step = event.get("step_update", {})
            if not initialized or step.get("conversation_id") != conversation:
                raise ValueError("step identity mismatch")
            if step.get("step_type") == "tool" and step.get("tool_name") != "finish":
                raise ValueError("unexpected tool invocation")
            if step.get("state") in {"ERROR", "CANCELED"}:
                raise ValueError("failed step")
        elif kind == "result":
            result = event.get("result", {})
            if not initialized or result.get("conversation_id") != conversation:
                raise ValueError("result identity mismatch")
            if result.get("status") != "SUCCESS":
                raise ValueError("unsuccessful result")
            decision = validate(result.get("structured_output"))
            completed = True
        else:
            raise ValueError("unknown stream event")
    if not completed:
        raise ValueError("missing terminal result")
    return decision


def normalize_model(model):
    if model is None:
        return None
    normalized = model.strip()
    if not normalized or normalized.lower() == "auto":
        return None
    if len(normalized) > 256:
        raise ValueError("invalid model")
    return normalized


def classify_child_failure(err):
    text = err.lower()
    if "timed out" in text or "timeout" in text:
        return "timeout"
    if "permission" in text or "denied" in text:
        return "permission"
    if "auth" in text or "token" in text or "logged in" in text:
        return "auth"
    return "child_exit"


def run(prompt, agy, timeout, model=None, supervised=False):
    model = normalize_model(model)
    binary = shutil.which(agy)
    if binary is None:
        raise ValueError("agy executable unavailable")
    binary = str(Path(binary).resolve())
    definition = Path(__file__).with_name("kg_decision_agent.md")
    with tempfile.TemporaryDirectory(prefix="kg-decision-") as root:
        target = Path(root) / ".agents/agents" / AGENT / "agent.md"
        target.parent.mkdir(parents=True)
        shutil.copyfile(definition, target)
        env = {k: v for k, v in os.environ.items()
               if not k.endswith("API_KEY") and k not in {"SURR_ENV_FILE", "GOOGLE_APPLICATION_CREDENTIALS"}}
        args = [binary, "--new-project", "--agent", AGENT, "--sandbox",
                "--disable-slash-commands", "--effort", "low", "--print-timeout", f"{timeout}s",
                "--output-format", "stream-json", "--json-schema", json.dumps(SCHEMA), "--print", prompt]
        if model is not None:
            args.extend(["--model", model])
        with tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err:
            child = subprocess.Popen(args, cwd=root, env=env, stdin=subprocess.DEVNULL,
                                     stdout=out, stderr=err, start_new_session=not supervised)
            deadline = time.monotonic() + timeout + 5
            try:
                while child.poll() is None:
                    if time.monotonic() >= deadline:
                        raise ValueError("runner timed out")
                    if os.fstat(out.fileno()).st_size + os.fstat(err.fileno()).st_size > MAX_BYTES:
                        raise ValueError("output too large")
                    time.sleep(0.05)
                if child.returncode != 0:
                    err.seek(0)
                    kind = classify_child_failure(err.read(4096).decode("utf-8", "replace"))
                    raise ValueError(
                        f"kind={kind} exit={child.returncode} pid={child.pid} "
                        f"stdout_bytes={os.fstat(out.fileno()).st_size} "
                        f"stderr_bytes={os.fstat(err.fileno()).st_size}"
                    )
                out.seek(0)
                return parse_stream(out.read(MAX_BYTES + 1))
            finally:
                if not supervised:
                    # The dedicated process group belongs only to this standalone invocation.
                    try:
                        os.killpg(child.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                child.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--agy", default="agy")
    parser.add_argument("--timeout", type=int, default=60)
    parser.add_argument("--model")
    parser.add_argument("--supervised", action="store_true",
                        help="Rust owns the shared Python/agy process group")
    args = parser.parse_args()
    try:
        prompt = sys.stdin.buffer.read(65537)
        if not prompt or len(prompt) > 65536 or not 1 <= args.timeout <= 300:
            raise ValueError("invalid input size or timeout")
        result = run(prompt.decode("utf-8"), args.agy, args.timeout, args.model,
                     args.supervised)
        print(json.dumps(result, ensure_ascii=False))
    except (ValueError, OSError) as exc:
        print(f"kg_decision: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
