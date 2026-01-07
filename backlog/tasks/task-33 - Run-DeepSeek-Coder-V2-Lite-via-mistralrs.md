# Task 33: Run DeepSeek-Coder-V2-Lite-Instruct via mistralrs-server

## Objective
Get `deepseek-ai/DeepSeek-Coder-V2-Lite-Instruct` running locally via `mistralrs-server` as the Scalpel backend.

---

## Why This Model

| Aspect | Details |
|--------|---------|
| **Size** | 2.4B active params (16B total, MoE) |
| **RAM** | ~5-6GB (Q4 quantized) |
| **Strengths** | Code-focused, 128K context, structured output |
| **Architecture** | Standard transformer (no VLM prefix issues) |
| **License** | MIT — no gating, no HF auth required |
| **Scalpel Fit** | Optimized for code tasks (parsing, linting, extraction) |

---

## Implementation Steps

### 1. Download Model
```bash
# Should auto-download on first run, or pre-fetch:
huggingface-cli download deepseek-ai/DeepSeek-Coder-V2-Lite-Instruct
```

### 2. Run Server
```bash
mistralrs-server --port 8111 --isq Q4K plain -m deepseek-ai/DeepSeek-Coder-V2-Lite-Instruct -a deepseek2
```

If `-a deepseek2` fails, try `-a llama`.

### 3. Verify Endpoint
```bash
curl http://localhost:8111/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "deepseek-ai/DeepSeek-Coder-V2-Lite-Instruct", "messages": [{"role": "user", "content": "Hello"}]}'
```

### 4. Update Scalpel Config
```bash
export SURR_SCALPEL_ENDPOINT=http://localhost:8111/v1/chat/completions
export SURR_SCALPEL_MODEL=deepseek-ai/DeepSeek-Coder-V2-Lite-Instruct
export SURR_SCALPEL_ENABLED=true
```

### 5. Test End-to-End
Call `scalpel` tool via surreal-mind and verify response.

---

## Context from Task 32 (Ministral 3B)

Task 32 attempted `mistralai/Ministral-3-3B-Instruct-2512` but failed due to VLM-style tensor naming (`language_model.model.*` prefix) that `mistralrs` couldn't parse. DeepSeek-Coder uses standard naming and should load cleanly.

---

## Files to Update on Success

| File | Update |
|------|--------|
| `src/clients/local.rs` | Default model ID (optional) |
| `walkthrough.md` | Scalpel setup instructions |
| `implementation_plan.md` | Scalpel configuration section |

---

## Acceptance Criteria

- [ ] `mistralrs-server` runs on port 8111 with DeepSeek-Coder loaded
- [ ] `curl` to `/v1/chat/completions` returns valid response
- [ ] Scalpel tool works end-to-end via surreal-mind
- [ ] RAM usage acceptable (~6GB or less)
