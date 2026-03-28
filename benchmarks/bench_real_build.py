"""
Real build benchmark: GPT agents build features on a shared Express API.

1. GPT generates a base Express API scaffold
2. Each agent gets a feature to BUILD (not rewrite): auth, search, webhooks, etc.
3. Each agent generates their code changes via GPT
4. Git side: worktrees (parallel work) + sequential merge into main
5. CRDT side: entity writes + single merge pass

This is what actually happens when multiple agents work on one project.
"""

import os
import json
import time
import shutil
import subprocess
import tempfile
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed

from openai import OpenAI

OPENAI_API_KEY = os.environ.get("OPENAI_API_KEY", "")
MODEL = "gpt-4.1"
CACHE_FILE = Path(__file__).parent / "build_rewrites.json"

client = OpenAI(api_key=OPENAI_API_KEY)

# ── Base project: a minimal Express API that agents will build on ──

BASE_APP = """\
import express from 'express';
import { db } from './db';

const app = express();
app.use(express.json());

// Health check
app.get('/health', (req, res) => {
    res.json({ status: 'ok', uptime: process.uptime() });
});

// Users CRUD
app.get('/users', async (req, res) => {
    const users = await db.query('SELECT * FROM users');
    res.json(users);
});

app.get('/users/:id', async (req, res) => {
    const user = await db.query('SELECT * FROM users WHERE id = ?', [req.params.id]);
    if (!user) return res.status(404).json({ error: 'User not found' });
    res.json(user);
});

app.post('/users', async (req, res) => {
    const { name, email } = req.body;
    const result = await db.insert('users', { name, email });
    res.status(201).json(result);
});

app.put('/users/:id', async (req, res) => {
    const updated = await db.update('users', req.params.id, req.body);
    if (!updated) return res.status(404).json({ error: 'User not found' });
    res.json(updated);
});

app.delete('/users/:id', async (req, res) => {
    const deleted = await db.delete('users', req.params.id);
    if (!deleted) return res.status(404).json({ error: 'User not found' });
    res.json({ success: true });
});

// Orders CRUD
app.get('/orders', async (req, res) => {
    const orders = await db.query('SELECT * FROM orders');
    res.json(orders);
});

app.get('/orders/:id', async (req, res) => {
    const order = await db.query('SELECT * FROM orders WHERE id = ?', [req.params.id]);
    if (!order) return res.status(404).json({ error: 'Order not found' });
    res.json(order);
});

app.post('/orders', async (req, res) => {
    const { userId, items, total } = req.body;
    const result = await db.insert('orders', { userId, items, total, status: 'pending' });
    res.status(201).json(result);
});

app.put('/orders/:id', async (req, res) => {
    const updated = await db.update('orders', req.params.id, req.body);
    if (!updated) return res.status(404).json({ error: 'Order not found' });
    res.json(updated);
});

// Products CRUD
app.get('/products', async (req, res) => {
    const products = await db.query('SELECT * FROM products');
    res.json(products);
});

app.get('/products/:id', async (req, res) => {
    const product = await db.query('SELECT * FROM products WHERE id = ?', [req.params.id]);
    if (!product) return res.status(404).json({ error: 'Product not found' });
    res.json(product);
});

app.post('/products', async (req, res) => {
    const { name, price, description } = req.body;
    const result = await db.insert('products', { name, price, description });
    res.status(201).json(result);
});

app.put('/products/:id', async (req, res) => {
    const updated = await db.update('products', req.params.id, req.body);
    if (!updated) return res.status(404).json({ error: 'Product not found' });
    res.json(updated);
});

// Start server
const PORT = process.env.PORT || 3000;
app.listen(PORT, () => {
    console.log(`Server running on port ${PORT}`);
});

export default app;
"""

# ── Features for agents to build ──
# Each agent modifies the SAME app.ts file (realistic: shared entry point)

AGENT_FEATURES = [
    {"name": "auth-middleware", "prompt": "Add JWT authentication middleware. Add a verifyToken middleware that checks Authorization header for Bearer token, verifies with jwt.verify(), attaches decoded user to req.user. Apply to all routes except /health and POST /users. Add POST /login route with email+password, bcrypt.compare(), returns JWT. Import jsonwebtoken and bcrypt. Keep all existing routes."},
    {"name": "input-validation", "prompt": "Add input validation to all POST and PUT routes. POST /users: validate name non-empty, email regex. POST /orders: userId number, items non-empty array, total positive. POST /products: name non-empty, price positive. PUT routes: at least one field. Return 400 with errors. Add validateBody helper. Keep all existing routes."},
    {"name": "error-handling", "prompt": "Add error handling. Wrap every route in try/catch logging error with timestamp, route, method, stack trace, returns 500. Add global error middleware (4 params). Add AppError class with statusCode and message. Replace 404s with throw new AppError. Keep all existing routes."},
    {"name": "request-logging", "prompt": "Add request logging middleware. Log every request with timestamp, method, URL, IP, user-agent, body. Log response status and duration on res.on('finish'). Add formatLog helper. Keep all existing routes."},
    {"name": "rate-limiting", "prompt": "Add rate limiting. Track requests per IP in a Map. 100 req/min/IP. Return 429 with Retry-After. Cleanup expired entries every 5min. Apply to all except /health. Keep all existing routes."},
    {"name": "pagination", "prompt": "Add pagination to GET list routes. Accept page and limit params (defaults 1, 20, max 100). COUNT query for total, LIMIT/OFFSET for data. Return { data, pagination }. Add parsePagination helper. Keep all existing routes."},
    {"name": "search-filter", "prompt": "Add search/filtering to GET list routes. /users: ?search= on name/email. /orders: ?status=, ?userId=. /products: ?search= on name/description, ?minPrice=, ?maxPrice=. Add buildWhereClause helper. Keep all existing routes."},
    {"name": "response-caching", "prompt": "Add response caching. CacheStore class with get/set/invalidate/cleanup. Cache /users 60s, /products 300s, /orders 30s. Individual GETs 10s. POST/PUT/DELETE invalidate cache. Add cache-control headers. Keep all existing routes."},
    {"name": "cors-security", "prompt": "Add CORS and security headers. cors middleware for preflight OPTIONS, configurable origins. securityHeaders middleware for X-Content-Type-Options, X-Frame-Options, HSTS, CSP. Apply before all routes. Keep all existing routes."},
    {"name": "graceful-shutdown", "prompt": "Add graceful shutdown. Track in-flight requests with counter middleware. On SIGTERM/SIGINT: stop connections, wait for requests, close db, exit 0. Store server ref. 30s force-exit timeout. Keep all existing routes."},
    {"name": "api-versioning", "prompt": "Add API versioning. Create a versionRouter that mounts all existing routes under /v1/. Add a /v2/ mount point that aliases to v1 for now. Add an apiVersion middleware that reads Accept-Version header or URL prefix. Add a deprecation warning header for v1. Keep all existing routes working under both /v1/ and root paths."},
    {"name": "request-id", "prompt": "Add request ID tracking. Generate a UUID for each request if no X-Request-ID header present. Attach it to req.id and include in all response headers. Include request ID in all error responses and log output. Add a generateRequestId helper. Keep all existing routes."},
    {"name": "soft-delete", "prompt": "Add soft delete to all DELETE routes. Instead of actual deletion, set a deleted_at timestamp. Modify GET routes to exclude soft-deleted records by default. Add ?includeDeleted=true query param to show them. Add PATCH /users/:id/restore, /orders/:id/restore, /products/:id/restore routes. Keep all existing routes."},
    {"name": "audit-logging", "prompt": "Add audit logging. Log every create, update, and delete operation to an audit_log table via db.insert('audit_log', {...}). Include: action, entity_type, entity_id, user_ip, timestamp, old_value, new_value. Add an auditLog helper function. Add GET /audit-log route to query audit entries. Keep all existing routes."},
    {"name": "response-compression", "prompt": "Add response compression middleware. Check Accept-Encoding header for gzip and deflate. Compress responses larger than 1KB. Use zlib.createGzip() and zlib.createDeflate(). Set Content-Encoding header. Skip compression for already-compressed content types. Keep all existing routes."},
    {"name": "request-timeout", "prompt": "Add request timeout middleware. Set a 30-second timeout for all routes. If a request exceeds the timeout, return 408 Request Timeout. Use setTimeout and clearTimeout. Add a configurable TIMEOUT_MS constant. Log timeout events. Keep all existing routes."},
    {"name": "api-key-auth", "prompt": "Add API key authentication as an alternative to JWT. Check X-API-Key header against a stored set of valid keys. Add GET /api-keys route (admin only) and POST /api-keys to generate new keys. Store keys in an in-memory Map with metadata (created_at, name, permissions). Keep all existing routes."},
    {"name": "webhook-support", "prompt": "Add webhook support. Add POST /webhooks to register a webhook URL for events (user.created, order.created, etc.). Add a triggerWebhook helper that POSTs to registered URLs when events happen. Call triggerWebhook from POST /users, POST /orders, POST /products routes. Add GET /webhooks to list registered webhooks. Keep all existing routes."},
    {"name": "batch-operations", "prompt": "Add batch operation endpoints. Add POST /users/batch to create multiple users at once. Add POST /orders/batch for bulk order creation. Add DELETE /users/batch with array of IDs. Add a processBatch helper that handles errors per-item and returns { succeeded, failed } counts. Keep all existing routes."},
    {"name": "field-selection", "prompt": "Add field selection to all GET routes. Accept ?fields=name,email query param. Only return requested fields in response. Add a selectFields helper that filters object properties. Apply to both list and individual GET routes. Default to all fields if not specified. Keep all existing routes."},
    {"name": "etag-support", "prompt": "Add ETag support for conditional requests. Generate ETags from response content using MD5 hash. Check If-None-Match header and return 304 Not Modified when matched. Add ETag and Last-Modified headers to all GET responses. Add a generateETag helper. Keep all existing routes."},
    {"name": "request-sanitization", "prompt": "Add request body sanitization. Strip HTML tags from all string fields in request bodies. Trim whitespace. Escape SQL special characters. Add a sanitizeBody middleware that recursively cleans all string values. Apply to POST and PUT routes. Keep all existing routes."},
    {"name": "response-envelope", "prompt": "Add a standard response envelope to all routes. Wrap all responses in { success: true/false, data: ..., error: ..., timestamp: ..., requestId: ... }. Add a sendSuccess and sendError helper attached to res. Modify all route handlers to use the helpers. Keep all existing routes."},
    {"name": "health-detailed", "prompt": "Enhance the /health endpoint. Add database connectivity check, memory usage (process.memoryUsage()), uptime, version from package.json, and response time of db.query('SELECT 1'). Return { status, checks: { database, memory, uptime }, timestamp }. Add a runHealthChecks async function. Keep all existing routes."},
    {"name": "idempotency", "prompt": "Add idempotency support for POST routes. Accept Idempotency-Key header. Store request results in an in-memory Map keyed by the idempotency key. If same key is sent again, return the cached result without re-executing. Expire cached results after 24 hours. Add IdempotencyStore class. Keep all existing routes."},
    {"name": "sorting", "prompt": "Add sorting to all GET list routes. Accept ?sort=field and ?order=asc|desc query params. For /users: allow sorting by name, email, created_at. For /orders: sort by total, status, created_at. For /products: sort by name, price. Add a buildOrderClause helper. Default sort by created_at desc. Keep all existing routes."},
    {"name": "bulk-update", "prompt": "Add bulk update endpoints. Add PATCH /users/bulk that accepts array of {id, ...fields} and updates multiple users in one request. Same for /orders/bulk and /products/bulk. Return count of updated records and any failures. Add a processBulkUpdate helper. Keep all existing routes."},
    {"name": "response-time-header", "prompt": "Add X-Response-Time header to all responses. Measure time from request start to response send using process.hrtime(). Add as middleware before all routes. Include in both success and error responses. Format as milliseconds with 2 decimal places. Keep all existing routes."},
    {"name": "content-negotiation", "prompt": "Add content negotiation. Check Accept header and return JSON (default), XML, or CSV based on what the client requests. Add toXml and toCsv helper functions. For XML, wrap in simple tags. For CSV, flatten objects to comma-separated. Apply to all GET routes. Keep all existing routes."},
    {"name": "retry-after", "prompt": "Add Retry-After headers when the server is overloaded. Track concurrent request count. When above 1000 concurrent, return 503 Service Unavailable with Retry-After header. Add a concurrencyTracker middleware. Add a /metrics endpoint showing current concurrent requests. Keep all existing routes."},
    {"name": "query-complexity", "prompt": "Add query complexity limiting. For list endpoints, calculate a complexity score based on: number of filters + sort fields + page size. Reject queries with complexity > 10 with 400 Bad Request. Add a calculateComplexity helper. Include complexity score in response headers. Keep all existing routes."},
    {"name": "cascade-delete", "prompt": "Add cascade delete logic. When deleting a user, also delete their orders. When deleting a product, update orders that reference it. Add a cascadeDelete helper that looks up related records. Log cascade operations. Return count of cascaded deletions in response. Keep all existing routes."},
    {"name": "data-export", "prompt": "Add data export endpoints. Add GET /users/export, /orders/export, /products/export that return all records as downloadable JSON. Set Content-Disposition header for file download. Add ?format=csv option. Add a streamResults helper for large datasets. Keep all existing routes."},
    {"name": "request-dedup", "prompt": "Add request deduplication. Track recent identical requests (same method + path + body hash) within a 5-second window. Return cached response for duplicates instead of re-executing. Add a RequestDeduplicator class with a Map and cleanup interval. Keep all existing routes."},
    {"name": "circuit-breaker", "prompt": "Add a circuit breaker pattern for database calls. Track consecutive failures. After 5 failures, open the circuit and return 503 for 30 seconds without hitting the db. After 30s, allow one test request (half-open). On success, close the circuit. Add CircuitBreaker class. Wrap all db.query calls. Keep all existing routes."},
    {"name": "feature-flags", "prompt": "Add feature flags support. Create a featureFlags Map at the top with flags like 'new_search', 'beta_orders', 'v2_users'. Add a checkFeature middleware. Add GET /features and PUT /features/:name routes to read and toggle flags. Add isFeatureEnabled helper used in route handlers. Keep all existing routes."},
    {"name": "request-throttle", "prompt": "Add request throttling per route. Allow configuring max concurrent requests per endpoint (e.g., POST /orders max 10 concurrent). Queue excess requests and process when a slot opens. Return 429 if queue exceeds 100. Add a ThrottleManager class. Keep all existing routes."},
    {"name": "data-masking", "prompt": "Add response data masking. Mask sensitive fields in responses: email addresses (show first 2 chars + ***@domain), phone numbers, credit card numbers. Add a maskSensitiveData helper that recursively checks field names. Apply to all GET responses. Keep all existing routes."},
    {"name": "multi-tenant", "prompt": "Add multi-tenant support. Read X-Tenant-ID header from requests. Add tenant_id to all database queries as a WHERE condition. Add a tenantMiddleware that validates the tenant header. Reject requests without a valid tenant. Keep all existing routes but scope them to the tenant."},
    {"name": "ab-testing", "prompt": "Add A/B testing support. Add a GET /experiments endpoint. Add an assignExperiment middleware that assigns users to experiment variants based on a hash of their IP. Include experiment variant in response headers. Add a trackExperiment helper. Keep all existing routes."},
    {"name": "sse-events", "prompt": "Add Server-Sent Events endpoint. Add GET /events that keeps connection open and streams events. When a user/order/product is created or updated, push an event to all connected clients. Track connected clients in a Set. Add a broadcastEvent helper called from POST and PUT routes. Keep all existing routes."},
    {"name": "dry-run", "prompt": "Add dry-run support for mutation routes. Accept ?dryRun=true query param on POST, PUT, DELETE routes. When enabled, validate the request and return what would happen without actually executing the db operation. Add a dryRunMiddleware. Return { dryRun: true, wouldAffect: ... } in response. Keep all existing routes."},
    {"name": "changelog", "prompt": "Add automatic changelog tracking. For every PUT and DELETE, store the before and after state in a changelog table. Add GET /users/:id/changelog, /orders/:id/changelog, /products/:id/changelog to view history. Add a recordChange helper. Keep all existing routes."},
    {"name": "dependency-check", "prompt": "Add dependency checking before delete. Before deleting a user, check if they have orders. Before deleting a product, check if any orders reference it. Return 409 Conflict with details of dependent records if dependencies exist. Add a checkDependencies helper. Keep all existing routes."},
    {"name": "auto-retry", "prompt": "Add automatic retry logic for failed database queries. Wrap db.query calls with a retry helper that retries up to 3 times with exponential backoff (100ms, 200ms, 400ms). Log retry attempts. Only retry on transient errors (connection timeout, deadlock). Add a withRetry helper function. Keep all existing routes."},
    {"name": "response-signing", "prompt": "Add response signing. Generate an HMAC-SHA256 signature of the response body using a secret key. Include the signature in X-Response-Signature header. Add a signResponse middleware. Add a SIGNING_SECRET constant. Useful for webhook verification. Keep all existing routes."},
    {"name": "ip-allowlist", "prompt": "Add IP allowlist middleware. Maintain a Set of allowed IP addresses/CIDR ranges. Add GET /allowlist and POST /allowlist routes (admin only). Block requests from non-allowed IPs with 403. Add an isIpAllowed helper that supports CIDR matching. Apply to all routes except /health. Keep all existing routes."},
    {"name": "conditional-update", "prompt": "Add conditional updates (optimistic locking). Add an updated_at field check on all PUT routes. Accept If-Unmodified-Since header. If the record was modified since that time, return 412 Precondition Failed. Add a checkPrecondition helper. Include Last-Modified header in GET responses. Keep all existing routes."},
    {"name": "query-logging", "prompt": "Add database query logging. Wrap all db.query, db.insert, db.update, db.delete calls to log: query text, params, duration, result count. Add a queryLogger wrapper function. Add GET /debug/queries endpoint that returns recent queries (last 100). Store in a circular buffer. Keep all existing routes."},
    {"name": "warmup-cache", "prompt": "Add cache warming on startup. After app.listen, preload frequently accessed data into an in-memory cache: all products, active user count, recent orders summary. Add a warmupCache async function called after server starts. Refresh every 5 minutes with setInterval. Add GET /cache/stats to show cache hit/miss ratios. Keep all existing routes."},
]


def get_agent_rewrite(agent_idx: int, base_code: str) -> dict:
    """Ask GPT to add a feature to the app."""
    feature = AGENT_FEATURES[agent_idx % len(AGENT_FEATURES)]

    response = client.chat.completions.create(
        model=MODEL,
        messages=[
            {
                "role": "system",
                "content": "You are a senior TypeScript developer. Modify the given Express app according to the task. Return ONLY the complete modified file, no explanation, no markdown fences. Keep ALL existing code and add your changes.",
            },
            {
                "role": "user",
                "content": f"Task: {feature['prompt']}\n\nCurrent app.ts:\n```\n{base_code}\n```",
            },
        ],
        max_tokens=4000,
        temperature=0.7,
    )

    return {
        "agent_idx": agent_idx,
        "feature": feature["name"],
        "prompt": feature["prompt"][:100],
        "rewritten_file": response.choices[0].message.content.strip(),
        "model": MODEL,
        "tokens_in": response.usage.prompt_tokens,
        "tokens_out": response.usage.completion_tokens,
    }


def generate_all_rewrites():
    """Get all agent rewrites. Cached to disk."""
    if CACHE_FILE.exists():
        print(f"Loading cached rewrites from {CACHE_FILE}")
        with open(CACHE_FILE) as f:
            return json.load(f)

    num_agents = len(AGENT_FEATURES)
    print(f"Generating {num_agents} feature builds via {MODEL} (this may take a minute)...")
    rewrites = []
    total_in = 0
    total_out = 0

    with ThreadPoolExecutor(max_workers=5) as executor:
        futures = {}
        for i in range(num_agents):
            future = executor.submit(get_agent_rewrite, i, BASE_APP)
            futures[future] = i

        for future in as_completed(futures):
            idx = futures[future]
            try:
                result = future.result()
                rewrites.append(result)
                total_in += result["tokens_in"]
                total_out += result["tokens_out"]
                print(f"  Agent {idx}: {result['feature']} [{result['tokens_out']} tokens]")
            except Exception as e:
                print(f"  Agent {idx}: FAILED - {e}")
                rewrites.append({
                    "agent_idx": idx,
                    "feature": AGENT_FEATURES[idx]["name"],
                    "prompt": "",
                    "rewritten_file": "",
                    "model": MODEL,
                    "tokens_in": 0,
                    "tokens_out": 0,
                    "error": str(e),
                })

    rewrites.sort(key=lambda r: r["agent_idx"])
    print(f"\nTotal: {total_in} tokens in, {total_out} tokens out")

    with open(CACHE_FILE, "w") as f:
        json.dump(rewrites, f, indent=2)

    return rewrites


def git_cmd(cwd, args, check=False):
    """Run a git command."""
    result = subprocess.run(
        ["git"] + args,
        cwd=cwd,
        capture_output=True,
        text=True,
        env={
            **os.environ,
            "GIT_AUTHOR_NAME": "test",
            "GIT_AUTHOR_EMAIL": "test@test.com",
            "GIT_COMMITTER_NAME": "test",
            "GIT_COMMITTER_EMAIL": "test@test.com",
        },
    )
    if check and result.returncode != 0:
        print(f"  git {' '.join(args)} failed: {result.stderr[:200]}")
    return result.returncode == 0


def bench_git_worktrees(rewrites: list, num_agents: int) -> tuple:
    """
    Git worktree workflow (how real multi-agent setups work):
    1. Create main repo with base app
    2. Create N worktrees (one per agent) - PARALLEL
    3. Each agent writes their modified file in their worktree
    4. Each agent commits in their worktree
    5. Merge all branches into main - SEQUENTIAL
    """
    agents = rewrites[:num_agents]
    tmp = Path(tempfile.mkdtemp(prefix=f"bench_wt_{num_agents}_"))
    main_repo = tmp / "main"

    try:
        # Setup: create main repo
        main_repo.mkdir()
        git_cmd(main_repo, ["init"])
        git_cmd(main_repo, ["checkout", "-b", "main"])
        (main_repo / "app.ts").write_text(BASE_APP)
        git_cmd(main_repo, ["add", "app.ts"])
        git_cmd(main_repo, ["commit", "-m", "initial scaffold"])

        # ── TIMED SECTION ──
        start = time.perf_counter()

        # 1. Create worktrees (parallel in real life, sequential here to measure overhead)
        worktree_paths = []
        for i in range(num_agents):
            branch = f"agent-{i}-{agents[i]['feature']}"
            wt_path = tmp / f"worktree-{i}"
            git_cmd(main_repo, ["branch", branch])
            git_cmd(main_repo, ["worktree", "add", str(wt_path), branch])
            worktree_paths.append((wt_path, branch))

        # 2. Each agent writes their modified file (simulates agent finishing work)
        for i, (wt_path, branch) in enumerate(worktree_paths):
            rewritten = agents[i].get("rewritten_file", "")
            if rewritten:
                # Strip markdown fences if GPT added them
                if rewritten.startswith("```"):
                    lines = rewritten.split("\n")
                    rewritten = "\n".join(lines[1:-1]) if lines[-1].strip() == "```" else "\n".join(lines[1:])
                (wt_path / "app.ts").write_text(rewritten)
            git_cmd(wt_path, ["add", "app.ts"])
            git_cmd(wt_path, ["commit", "-m", f"agent-{i}: {agents[i]['feature']}"])

        # 3. Remove worktrees before merging
        for wt_path, branch in worktree_paths:
            git_cmd(main_repo, ["worktree", "remove", str(wt_path), "--force"])

        # 4. Sequential merge into main
        git_cmd(main_repo, ["checkout", "main"])
        clean = 0
        for i, (_, branch) in enumerate(worktree_paths):
            if git_cmd(main_repo, ["merge", branch, "--no-edit"]):
                clean += 1
            else:
                git_cmd(main_repo, ["merge", "--abort"])

        elapsed = time.perf_counter() - start
        return (int(elapsed * 1000), clean)

    finally:
        # Clean up worktrees first
        try:
            result = subprocess.run(
                ["git", "worktree", "list"],
                cwd=main_repo,
                capture_output=True, text=True
            )
        except:
            pass
        shutil.rmtree(tmp, ignore_errors=True)


def bench_crdt(rewrites: list, num_agents: int) -> tuple:
    """
    CRDT workflow: save agent outputs to JSON, run Rust benchmark.
    """
    agents = rewrites[:num_agents]

    rewrite_file = Path(tempfile.mktemp(suffix=".json", prefix=f"crdt_build_{num_agents}_"))
    data = {
        "base_file": BASE_APP,
        "num_agents": num_agents,
        "rewrites": [
            {
                "agent_idx": a["agent_idx"],
                "func_idx": a["agent_idx"],  # not used in this mode
                "rewritten": a.get("rewritten_file", ""),
            }
            for a in agents
        ],
    }
    rewrite_file.write_text(json.dumps(data))

    try:
        weave_dir = Path(__file__).parent.parent
        result = subprocess.run(
            [
                "cargo", "test", "-p", "weave-crdt", "--test", "bench_build_llm",
                "--release", "--", "--nocapture", "bench_crdt_build",
            ],
            cwd=weave_dir,
            capture_output=True,
            text=True,
            env={**os.environ, "CRDT_REWRITE_FILE": str(rewrite_file)},
            timeout=60,
        )

        for line in result.stderr.split("\n"):
            if line.startswith("CRDT_RESULT:"):
                parts = line.split(":")
                return (int(parts[1]), int(parts[2]))

        print(f"  CRDT output: {result.stderr[-300:]}")
        return (0, 0)

    finally:
        rewrite_file.unlink(missing_ok=True)


def main():
    line_count = BASE_APP.count("\n")
    print(f"\n{'='*60}")
    print(f"REAL BUILD BENCHMARK: GPT agents build features")
    print(f"{'='*60}")
    print(f"Base: Express API scaffold ({line_count} lines)")
    print(f"Each agent adds a complete feature to the same app.ts")
    print(f"Model: {MODEL}")
    print(f"Git: worktrees (parallel work) + sequential merge")
    print(f"CRDT: entity writes + single merge pass")
    print()

    # Get LLM feature builds
    rewrites = generate_all_rewrites()
    valid = sum(1 for r in rewrites if r.get("rewritten_file"))
    print(f"\n{valid}/{len(rewrites)} valid feature builds")

    # Show what each agent built
    print("\nAgents:")
    for r in rewrites:
        tokens = r.get("tokens_out", 0)
        lines = r.get("rewritten_file", "").count("\n")
        print(f"  Agent {r['agent_idx']}: {r['feature']} ({lines} lines, {tokens} tokens)")

    print(f"\n{'Agents':<8} {'Git(ms)':>12} {'CRDT(ms)':>12} {'Ratio':>8} {'Git merges':>12} {'CRDT clean':>12}")

    for num_agents in [2, 5, 10, 20, 50]:
        git_ms, git_clean = bench_git_worktrees(rewrites, num_agents)
        crdt_ms, crdt_clean = bench_crdt(rewrites, num_agents)

        ratio = git_ms / max(crdt_ms, 1)
        print(
            f"{num_agents:<8} {git_ms:>10} ms {crdt_ms:>10} ms {ratio:>7.1f}x "
            f"{f'{git_clean}/{num_agents}':>12} {f'{crdt_clean}/{num_agents}':>12}"
        )


if __name__ == "__main__":
    main()
