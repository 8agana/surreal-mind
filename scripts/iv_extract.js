#!/usr/bin/env node

// scripts/iv_extract.js
// One-shot extractor: tries Gemini CLI models in order, then optional Grok HTTP fallback.
// Outputs strict JSON to --out (or stdout with '-') and emits compact telemetry to stderr.

const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');

const { buildPrompt, safeJsonParse, validateExtraction } = require('../lib/iv_utils');

async function run() {
  const args = parseArgs(process.argv.slice(2));
  if (!args.input) {
    console.error(JSON.stringify({ stage: 'error', reason: 'missing_input' }));
    process.exit(2);
  }
  const outToStdout = !args.out || args.out === '-';

  const raw = fs.readFileSync(args.input, 'utf8');
  let input;
  try { input = JSON.parse(raw); } catch (e) {
    console.error(JSON.stringify({ stage: 'error', reason: 'invalid_input_json' }));
    process.exit(2);
  }

  const prompt = buildPrompt(input);

  const models = (process.env.IV_MODELS || 'gemini-2.5-pro,gemini-2.5-flash')
    .split(',').map(s => s.trim()).filter(Boolean);
  const cliCmd = process.env.IV_CLI_CMD || 'gemini';
  const cliArgsJson = process.env.IV_CLI_ARGS_JSON || '["generate","--model","{model}","--response-mime-type","application/json","--temperature","0"]';
  let cliArgs;
  try { cliArgs = JSON.parse(cliArgsJson); } catch { cliArgs = ["generate","--model","{model}","--response-mime-type","application/json","--temperature","0"]; }
  // Be tolerant by default for CLI output; one repair pass will be attempted regardless
  const strictJson = (process.env.IV_STRICT_JSON || 'false').toLowerCase() !== 'false';
  const perTimeoutMs = parseInt(process.env.IV_TIMEOUT_MS || '20000', 10);
  const totalTimeoutMs = parseInt(process.env.IV_TOTAL_TIMEOUT_MS || '30000', 10);
  const allowGrok = (process.env.IV_ALLOW_GROK || 'true').toLowerCase() !== 'false';
  const maxEntities = parseInt(process.env.IV_MAX_ENTITIES || '200', 10);
  const maxEdges = parseInt(process.env.IV_MAX_EDGES || '300', 10);

  const deadline = Date.now() + totalTimeoutMs;

  let lastError = null;
  // Try Gemini CLI models
  for (const model of models) {
    const rem = Math.max(1000, deadline - Date.now());
    const timeout = Math.min(perTimeoutMs, rem);
    const start = Date.now();
    const argsModel = cliArgs.map(a => a === '{model}' ? model : a);
    const res = await runCli(cliCmd, argsModel, prompt, timeout);
    const latency = Date.now() - start;
    if (!res.ok) {
      lastError = res.reason || 'cli_failed';
      console.error(JSON.stringify({ stage: 'attempt', provider: model, status: 'fail', latency_ms: latency, reason: lastError }));
      continue;
    }
    console.error(JSON.stringify({ stage: 'attempt', provider: model, status: 'ok', latency_ms: latency, bytes_out: res.stdout.length }));
    let obj = safeJsonParse(res.stdout, strictJson);
    if (!obj) {
      console.error(JSON.stringify({ stage: 'attempt', provider: model, status: 'fail', reason: 'invalid_json' }));
      continue;
    }
    // Inject doc_meta and enforce caps
    obj.doc_meta = obj.doc_meta || {};
    obj.doc_meta.origin = 'inner_voice';
    obj.doc_meta.model = model;
    obj.doc_meta.latency_ms = latency;
    const e = Array.isArray(obj.entities) ? obj.entities : [];
    const r = Array.isArray(obj.edges) ? obj.edges : [];
    let truncated = false;
    if (e.length > maxEntities) { obj.entities = e.slice(0, maxEntities); truncated = true; }
    if (r.length > maxEdges) { obj.edges = r.slice(0, maxEdges); truncated = true; }
    if (truncated) obj.truncated = true;

    // Optional AJV validation if available
    let valid = { ok: true };
    try {
      const Ajv = require('ajv');
      const ajv = new Ajv({ allErrors: true, strict: false });
      const schemaPath = path.join(__dirname, '..', 'schemas', 'kg_extraction.schema.json');
      const schema = JSON.parse(fs.readFileSync(schemaPath, 'utf8'));
      const validate = ajv.compile(schema);
      if (!validate(obj)) {
        valid = { ok: false, reason: 'ajv_invalid', errors: validate.errors };
      }
    } catch (_) {
      // ajv not present; fall back to lightweight validation
      valid = validateExtraction(obj);
    }
    if (!valid.ok) {
      console.error(JSON.stringify({ stage: 'attempt', provider: model, status: 'fail', reason: valid.reason || 'schema_invalid' }));
      continue;
    }
    // Success
    if (outToStdout) process.stdout.write(JSON.stringify(obj));
    else fs.writeFileSync(args.out, JSON.stringify(obj));
    console.error(JSON.stringify({ stage: 'result', provider: model, entities: (obj.entities||[]).length, edges: (obj.edges||[]).length, truncated: !!obj.truncated, latency_ms: latency }));
    process.exit(0);
  }

  // Fallback to Grok if allowed
  if (allowGrok && process.env.GROK_API_KEY) {
    const start = Date.now();
    try {
      const out = await callGrok(prompt);
      const latency = Date.now() - start;
      let obj = safeJsonParse(out, strictJson);
      if (!obj) throw new Error('invalid_json');
      // Inject doc_meta and caps for Grok as well
      obj.doc_meta = obj.doc_meta || {};
      obj.doc_meta.origin = 'inner_voice';
      obj.doc_meta.model = 'grok-code-fast-1';
      obj.doc_meta.latency_ms = latency;
      const e = Array.isArray(obj.entities) ? obj.entities : [];
      const r = Array.isArray(obj.edges) ? obj.edges : [];
      let truncated = false;
      if (e.length > maxEntities) { obj.entities = e.slice(0, maxEntities); truncated = true; }
      if (r.length > maxEdges) { obj.edges = r.slice(0, maxEdges); truncated = true; }
      if (truncated) obj.truncated = true;

      // Optional AJV validation
      let valid = { ok: true };
      try {
        const Ajv = require('ajv');
        const ajv = new Ajv({ allErrors: true, strict: false });
        const schemaPath = path.join(__dirname, '..', 'schemas', 'kg_extraction.schema.json');
        const schema = JSON.parse(fs.readFileSync(schemaPath, 'utf8'));
        const validate = ajv.compile(schema);
        if (!validate(obj)) {
          valid = { ok: false, reason: 'ajv_invalid', errors: validate.errors };
        }
      } catch (_) {
        valid = validateExtraction(obj);
      }
      if (!valid.ok) throw new Error(valid.reason || 'schema_invalid');

      if (outToStdout) process.stdout.write(JSON.stringify(obj));
      else fs.writeFileSync(args.out, JSON.stringify(obj));
      console.error(JSON.stringify({ stage: 'result', provider: 'grok-code-fast-1', entities: (obj.entities||[]).length, edges: (obj.edges||[]).length, truncated: !!obj.truncated, latency_ms: latency }));
      process.exit(0);
    } catch (e) {
      console.error(JSON.stringify({ stage: 'attempt', provider: 'grok-code-fast-1', status: 'fail', reason: e.message || 'grok_failed' }));
    }
  }

  // Total failure: emit fatal telemetry and exit non-zero
  console.error(JSON.stringify({ stage: 'fatal', reason: 'no_provider_succeeded' }));
  process.exit(3);
}

function parseArgs(argv) {
  const out = {}; let k;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--input') { out.input = argv[++i]; }
    else if (a === '--out') { out.out = argv[++i]; }
    else if (a === '--') { break; }
    else if (a.startsWith('--')) { k = a.slice(2); out[k] = true; }
  }
  return out;
}

function runCli(cmd, args, prompt, timeoutMs) {
  return new Promise((resolve) => {
    const child = spawn(cmd, args, { stdio: ['pipe', 'pipe', 'pipe'] });
    let out = ''; let err = '';
    const timer = setTimeout(() => {
      child.kill('SIGKILL');
      resolve({ ok: false, reason: 'timeout' });
    }, timeoutMs);
    child.stdout.on('data', d => out += d.toString());
    child.stderr.on('data', d => err += d.toString());
    child.on('error', e => { clearTimeout(timer); resolve({ ok: false, reason: 'spawn_error' }); });
    child.on('close', code => {
      clearTimeout(timer);
      if (code !== 0) return resolve({ ok: false, reason: 'non_zero_exit', code, err });
      resolve({ ok: true, stdout: out });
    });
    // Most CLIs read prompt from stdin
    child.stdin.write(prompt);
    child.stdin.end();
  });
}

async function callGrok(prompt) {
  const base = process.env.GROK_BASE_URL || 'https://api.x.ai/v1';
  const model = process.env.GROK_MODEL || 'grok-code-fast-1';
  const key = process.env.GROK_API_KEY;
  const messages = [
    { role: 'system', content: 'Output ONLY one JSON object matching the required schema. No prose.' },
    { role: 'user', content: prompt }
  ];
  const res = await fetch(`${base}/chat/completions`, {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${key}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({ model, messages })
  });
  if (!res.ok) throw new Error(`http_${res.status}`);
  const data = await res.json();
  const txt = data?.choices?.[0]?.message?.content || '';
  return txt.trim();
}

run().catch(e => {
  console.error(JSON.stringify({ stage: 'fatal', reason: e.message || 'unknown' }));
  process.exit(1);
});

