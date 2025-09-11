// lib/iv_utils.js

function buildPrompt(input) {
  const synth = (input && input.synth_text) ? String(input.synth_text) : '';
  const docId = (input && input.doc_id) ? String(input.doc_id) : '';
  const promptHash = (input && input.prompt_hash) ? String(input.prompt_hash) : '';
  const guide = [
    'Return ONLY one JSON object that matches this exact shape:',
    '{',
    '  "entities": [ { "id": "e1", "label": "Acme", "type": "ORG", "aliases": [], "confidence": 0.9 } ],',
    '  "edges": [ { "from_id": "e1", "to_id": "e2", "relation": "FOUNDED_BY", "confidence": 0.8 } ],',
    '  "doc_meta": { "origin": "inner_voice", "prompt_hash": "", "model": "", "latency_ms": 0 }',
    '}',
    '',
    'Rules:',
    '- ONE JSON object only, no markdown or prose.',
    '- confidences in [0,1].',
    '- ids must look like e1, e2, ... and edges must reference existing ids.',
    '- keep lists small and relevant.',
  ].join('\n');
  return `${guide}\n\nDOC_ID: ${docId}\nPROMPT_HASH: ${promptHash}\n\nTEXT:\n${synth}`;
}

function safeJsonParse(txt, strict) {
  if (typeof txt !== 'string') return null;
  let t = txt.trim();
  // Strip code fences if present
  if (t.startsWith('```')) {
    t = t.replace(/^```[a-zA-Z]*\n/, '').replace(/```\s*$/, '').trim();
  }
  try { return JSON.parse(t); } catch {}
  if (!strict) {
    // robust repair: extract first complete JSON object using brace matching
    const start = t.indexOf('{');
    if (start >= 0) {
      let depth = 0, end = -1;
      for (let i = start; i < t.length; i++) {
        const ch = t[i];
        if (ch === '{') depth++;
        else if (ch === '}') {
          depth--;
          if (depth === 0) { end = i; break; }
        }
      }
      if (end > start) {
        try { return JSON.parse(t.slice(start, end + 1)); } catch {}
      }
    }
  }
  return null;
}

function validateExtraction(obj) {
  if (!obj || typeof obj !== 'object') return { ok: false, reason: 'not_object' };
  if (!Array.isArray(obj.entities)) obj.entities = [];
  if (!Array.isArray(obj.edges)) obj.edges = [];
  // basic checks
  const ids = new Set();
  for (const e of obj.entities) {
    if (!e || typeof e !== 'object') return { ok: false, reason: 'entity_not_object' };
    if (typeof e.id !== 'string' || !/^e\d+$/.test(e.id)) return { ok: false, reason: 'bad_entity_id' };
    if (typeof e.label !== 'string' || !e.label.trim()) return { ok: false, reason: 'bad_entity_label' };
    if (e.confidence != null && (e.confidence < 0 || e.confidence > 1)) return { ok: false, reason: 'bad_entity_conf' };
    ids.add(e.id);
  }
  for (const r of obj.edges) {
    if (!r || typeof r !== 'object') return { ok: false, reason: 'edge_not_object' };
    if (!ids.has(r.from_id) || !ids.has(r.to_id)) return { ok: false, reason: 'edge_bad_ref' };
    if (typeof r.relation !== 'string' || !r.relation.trim()) return { ok: false, reason: 'edge_no_relation' };
    if (r.confidence != null && (r.confidence < 0 || r.confidence > 1)) return { ok: false, reason: 'edge_bad_conf' };
  }
  // doc_meta optional
  if (obj.doc_meta && typeof obj.doc_meta !== 'object') return { ok: false, reason: 'doc_meta_bad' };
  return { ok: true };
}

module.exports = { buildPrompt, safeJsonParse, validateExtraction };

