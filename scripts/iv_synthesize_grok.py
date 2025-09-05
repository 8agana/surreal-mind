#!/usr/bin/env python3
"""
Synthesize an answer from inner_voice snippets using xAI Grok.

Usage:
  - Pipe inner_voice RetrieveOut JSON to stdin, or pass a file path:
      inner_voice_json | ./scripts/iv_synthesize_grok.py
      ./scripts/iv_synthesize_grok.py path/to/retrieve_out.json

Env:
  GROK_API_KEY   (required)
  GROK_BASE_URL  (optional, default: https://api.x.ai/v1)
  GROK_MODEL     (optional, default: grok-2-latest)

This is intentionally small and dependency-light.
"""
import json, os, sys, textwrap
import urllib.request


def read_input() -> dict:
    if not sys.stdin.isatty():
        data = sys.stdin.read()
        if data.strip():
            return json.loads(data)
    if len(sys.argv) > 1:
        with open(sys.argv[1], 'r') as f:
            return json.load(f)
    print("Provide inner_voice RetrieveOut JSON via stdin or file path.", file=sys.stderr)
    sys.exit(2)


def build_prompt(payload: dict) -> list:
    snippets = payload.get("snippets") or []
    diagnostics = payload.get("diagnostics") or {}
    if not snippets:
        return [
            {"role": "system", "content": "You have no sources; reply: 'No grounded answer available.'"},
            {"role": "user", "content": "No snippets provided."},
        ]

    # Keep it concise: include up to 8 snippets
    max_snips = 8
    lines = []
    for i, sn in enumerate(snippets[:max_snips], 1):
        text = sn.get("text", "").strip()
        if len(text) > 800:
            text = text[:800]
        meta = f"[{i}] {sn.get('table','?')}:{sn.get('id','?')} score={sn.get('score',0):.3f}"
        lines.append(meta + "\n" + text)

    system = (
        "You are a careful, grounded synthesizer.\n"
        "Only use the provided snippets.\n"
        "Cite sources inline like [1], [2].\n"
        "Prefer concise answers (<= 4 sentences).\n"
        "If insufficient evidence, say so and request a clarifier."
    )
    user = (
        "Snippets:\n" + "\n\n".join(lines) +
        "\n\nTask: Provide a concise, grounded answer with inline [n] citations."
    )
    return [{"role": "system", "content": system}, {"role": "user", "content": user}]


def call_grok(messages: list) -> str:
    api_key = os.getenv("GROK_API_KEY")
    if not api_key:
        print("GROK_API_KEY not set", file=sys.stderr)
        sys.exit(2)
    base = os.getenv("GROK_BASE_URL", "https://api.x.ai/v1")
    model = os.getenv("GROK_MODEL", "grok-2-latest")

    url = f"{base.rstrip('/')}/chat/completions"
    body = json.dumps({
        "model": model,
        "messages": messages,
        "temperature": 0.2,
        "max_tokens": 400,
    }).encode("utf-8")

    req = urllib.request.Request(url, data=body, method="POST")
    req.add_header("Authorization", f"Bearer {api_key}")
    req.add_header("Content-Type", "application/json")

    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            data = json.loads(resp.read().decode("utf-8"))
    except Exception as e:
        print(f"Grok request failed: {e}", file=sys.stderr)
        sys.exit(1)

    # xAI returns OpenAI-style choices
    choices = data.get("choices") or []
    if choices and choices[0].get("message"):
        return choices[0]["message"].get("content", "").strip()
    return json.dumps(data, indent=2)


def main():
    payload = read_input()
    messages = build_prompt(payload)
    out = call_grok(messages)
    print(out)


if __name__ == "__main__":
    main()

