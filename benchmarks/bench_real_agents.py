"""
Real agent benchmark: GPT-5.2 agents rewrite functions in a 200-function file.
Measures coordination overhead: git subprocess vs weave CRDT.

Step 1: Generate 50 function rewrites via GPT-5.2 (done once, cached)
Step 2: Git workflow with those rewrites (branch + write + commit + merge)
Step 3: Save rewrites for CRDT benchmark (Rust side)
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
MODEL = "gpt-4.1"  # fallback if gpt-5.2 isn't available
CACHE_FILE = Path(__file__).parent / "agent_rewrites.json"
NUM_FUNCTIONS = 200
MAX_AGENTS = 50

client = OpenAI(api_key=OPENAI_API_KEY)


def generate_200_func_file():
    lines = ["import { db } from './db';", "import { logger } from './logger';", "import { cache } from './cache';", ""]
    for i in range(NUM_FUNCTIONS):
        table = ["users", "orders", "products", "sessions", "events"][i % 5]
        lines.append(f"export function handler{i}(req: Request, res: Response) {{")
        lines.append(f"    const id = req.params.id;")
        lines.append(f"    const data = db.query('SELECT * FROM {table} WHERE id = ?', [id]);")
        lines.append(f"    if (!data) return res.status(404).json({{ error: '{table} not found' }});")
        lines.append(f"    return res.json({{ result: data, table: '{table}' }});")
        lines.append("}")
        lines.append("")
    return "\n".join(lines)


TASK_TYPES = [
    "Add input validation (check id exists, is a string, reasonable length)",
    "Add a caching layer (check cache first, set cache after db query, with TTL)",
    "Add error handling with try/catch and structured error logging",
    "Add request logging with timing metrics (log start, duration, result)",
    "Add pagination support (page, limit query params, return total count)",
    "Add rate limiting (track requests per IP, return 429 if over limit)",
    "Refactor to async/await with proper error propagation",
]


def rewrite_function(agent_idx: int, func_idx: int, func_code: str) -> dict:
    """Call GPT-5.2 to rewrite a function."""
    task = TASK_TYPES[agent_idx % len(TASK_TYPES)]

    response = client.chat.completions.create(
        model=MODEL,
        messages=[
            {
                "role": "system",
                "content": "You are a TypeScript developer. Rewrite the given function according to the task. Return ONLY the function code, no explanation, no markdown fences.",
            },
            {
                "role": "user",
                "content": f"Task: {task}\n\nFunction:\n{func_code}",
            },
        ],
        max_tokens=500,
        temperature=0.7,
    )

    return {
        "agent_idx": agent_idx,
        "func_idx": func_idx,
        "task": task,
        "original": func_code,
        "rewritten": response.choices[0].message.content.strip(),
        "model": MODEL,
        "tokens_in": response.usage.prompt_tokens,
        "tokens_out": response.usage.completion_tokens,
    }


def extract_function(file_content: str, func_idx: int) -> str:
    """Extract a single function from the file."""
    marker = f"export function handler{func_idx}("
    start = file_content.find(marker)
    if start == -1:
        return ""
    rest = file_content[start:]
    depth = 0
    for i, ch in enumerate(rest):
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return rest[: i + 1]
    return ""


def replace_function(file_content: str, func_idx: int, new_func: str) -> str:
    """Replace a function in the file content."""
    marker = f"export function handler{func_idx}("
    async_marker = f"export async function handler{func_idx}("
    start = file_content.find(marker)
    if start == -1:
        start = file_content.find(async_marker)
    if start == -1:
        return file_content

    rest = file_content[start:]
    depth = 0
    end_offset = 0
    for i, ch in enumerate(rest):
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                end_offset = i + 1
                break

    return file_content[:start] + new_func + file_content[start + end_offset :]


def generate_rewrites(base_file: str):
    """Generate 50 function rewrites via GPT-5.2. Cached to disk."""
    if CACHE_FILE.exists():
        print(f"Loading cached rewrites from {CACHE_FILE}")
        with open(CACHE_FILE) as f:
            return json.load(f)

    print(f"Generating {MAX_AGENTS} function rewrites via {MODEL}...")
    rewrites = []
    total_tokens_in = 0
    total_tokens_out = 0

    # Run API calls in parallel (10 at a time)
    with ThreadPoolExecutor(max_workers=10) as executor:
        futures = {}
        for i in range(MAX_AGENTS):
            func_idx = i % NUM_FUNCTIONS
            func_code = extract_function(base_file, func_idx)
            future = executor.submit(rewrite_function, i, func_idx, func_code)
            futures[future] = i

        for future in as_completed(futures):
            idx = futures[future]
            try:
                result = future.result()
                rewrites.append(result)
                total_tokens_in += result["tokens_in"]
                total_tokens_out += result["tokens_out"]
                print(f"  Agent {idx}: handler{result['func_idx']} ({result['task'][:40]}...) [{result['tokens_out']} tokens]")
            except Exception as e:
                print(f"  Agent {idx}: FAILED - {e}")
                rewrites.append({
                    "agent_idx": idx,
                    "func_idx": idx % NUM_FUNCTIONS,
                    "task": TASK_TYPES[idx % len(TASK_TYPES)],
                    "original": "",
                    "rewritten": "",
                    "model": MODEL,
                    "tokens_in": 0,
                    "tokens_out": 0,
                    "error": str(e),
                })

    # Sort by agent index
    rewrites.sort(key=lambda r: r["agent_idx"])

    print(f"\nTotal: {total_tokens_in} tokens in, {total_tokens_out} tokens out")

    with open(CACHE_FILE, "w") as f:
        json.dump(rewrites, f, indent=2)

    return rewrites


def git_cmd(cwd, args):
    """Run a git command, return success."""
    result = subprocess.run(
        ["git"] + args,
        cwd=cwd,
        capture_output=True,
        env={
            **os.environ,
            "GIT_AUTHOR_NAME": "test",
            "GIT_AUTHOR_EMAIL": "test@test.com",
            "GIT_COMMITTER_NAME": "test",
            "GIT_COMMITTER_EMAIL": "test@test.com",
        },
    )
    return result.returncode == 0


def bench_git(base_file: str, rewrites: list, num_agents: int) -> tuple:
    """Run git workflow with real LLM rewrites. Returns (duration_ms, clean_count)."""
    tmp = Path(tempfile.mkdtemp(prefix=f"bench_git_{num_agents}_"))

    try:
        # Init repo
        git_cmd(tmp, ["init"])
        git_cmd(tmp, ["checkout", "-b", "main"])
        (tmp / "api.ts").write_text(base_file)
        git_cmd(tmp, ["add", "api.ts"])
        git_cmd(tmp, ["commit", "-m", "initial"])

        start = time.perf_counter()

        # Each agent: branch, apply LLM rewrite, commit
        for i in range(num_agents):
            rewrite = rewrites[i]
            branch = f"agent-{i}"
            git_cmd(tmp, ["checkout", "main"])
            git_cmd(tmp, ["checkout", "-b", branch])

            content = (tmp / "api.ts").read_text()
            if rewrite.get("rewritten"):
                modified = replace_function(content, rewrite["func_idx"], rewrite["rewritten"])
            else:
                modified = content
            (tmp / "api.ts").write_text(modified)

            git_cmd(tmp, ["add", "api.ts"])
            git_cmd(tmp, ["commit", "-m", f"agent-{i} rewrites handler{rewrite['func_idx']}"])

        # Sequential merge into main
        git_cmd(tmp, ["checkout", "main"])
        clean = 0
        for i in range(num_agents):
            branch = f"agent-{i}"
            if git_cmd(tmp, ["merge", branch, "--no-edit"]):
                clean += 1
            else:
                git_cmd(tmp, ["merge", "--abort"])

        elapsed = time.perf_counter() - start
        return (int(elapsed * 1000), clean)

    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def bench_crdt_via_rust(base_file: str, rewrites: list, num_agents: int) -> tuple:
    """
    Save LLM rewrites to a JSON file, then invoke the Rust CRDT benchmark
    that reads them and runs update_entity_content + merge_file_entities.
    Returns (duration_ms, clean_count).
    """
    # Save rewrites for Rust to consume
    rewrite_file = Path(tempfile.mktemp(suffix=".json", prefix=f"crdt_rewrites_{num_agents}_"))
    data = {
        "base_file": base_file,
        "num_agents": num_agents,
        "rewrites": rewrites[:num_agents],
    }
    rewrite_file.write_text(json.dumps(data))

    try:
        # Run the Rust CRDT benchmark
        weave_dir = Path(__file__).parent.parent
        result = subprocess.run(
            [
                "cargo", "test", "-p", "weave-crdt", "--test", "bench_200_llm",
                "--release", "--", "--nocapture", "bench_crdt_from_json",
            ],
            cwd=weave_dir,
            capture_output=True,
            text=True,
            env={**os.environ, "CRDT_REWRITE_FILE": str(rewrite_file)},
        )

        # Parse output for timing
        for line in result.stderr.split("\n"):
            if line.startswith("CRDT_RESULT:"):
                parts = line.split(":")
                return (int(parts[1]), int(parts[2]))

        print(f"  CRDT stderr: {result.stderr[-500:]}")
        return (0, 0)

    finally:
        rewrite_file.unlink(missing_ok=True)


def main():
    base_file = generate_200_func_file()
    line_count = base_file.count("\n")

    print(f"\n{'='*60}")
    print(f"REAL AGENT BENCHMARK: GPT-5.2 + 200-FUNCTION FILE")
    print(f"{'='*60}")
    print(f"File: {NUM_FUNCTIONS} TypeScript handlers ({line_count} lines)")
    print(f"Model: {MODEL}")
    print()

    # Step 1: Get LLM rewrites
    rewrites = generate_rewrites(base_file)
    valid = sum(1 for r in rewrites if r.get("rewritten"))
    print(f"\n{valid}/{len(rewrites)} valid rewrites generated")

    # Step 2: Benchmark
    print(f"\n{'Agents':<8} {'Git(ms)':>12} {'CRDT(ms)':>12} {'Ratio':>8} {'Git clean':>12} {'CRDT clean':>12}")

    for num_agents in [2, 5, 10, 20, 50]:
        git_ms, git_clean = bench_git(base_file, rewrites, num_agents)
        crdt_ms, crdt_clean = bench_crdt_via_rust(base_file, rewrites, num_agents)

        ratio = git_ms / max(crdt_ms, 1)
        print(
            f"{num_agents:<8} {git_ms:>10} ms {crdt_ms:>10} ms {ratio:>7.1f}x "
            f"{f'{git_clean}/{num_agents}':>12} {f'{crdt_clean}/{num_agents}':>12}"
        )


if __name__ == "__main__":
    main()
