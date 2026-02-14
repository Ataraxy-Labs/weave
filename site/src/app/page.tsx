import React from "react";

export default function Home() {
  return (
    <div className="min-h-screen">
      {/* Hero */}
      <section className="px-6 pt-20 pb-16 max-w-5xl mx-auto">
        <p className="text-sm tracking-widest text-gray-500 uppercase mb-4" style={{ fontFamily: "var(--font-heading)" }}>
          by <a href="https://ataraxy-labs.com" className="hover:text-gray-300 transition-colors">Ataraxy Labs</a>
        </p>
        <h1 className="text-5xl md:text-7xl font-bold mb-6 leading-tight" style={{ fontFamily: "var(--font-heading)" }}>
          Weave
        </h1>
        <p className="text-xl md:text-2xl text-gray-300 mb-4 max-w-3xl leading-relaxed">
          Entity-level semantic merge driver for Git. Resolves conflicts at the function and class level using tree-sitter.
        </p>
        <p className="text-lg text-gray-500 mb-10 max-w-2xl">
          Git merges lines. Weave merges entities. Two developers editing different functions in the same file? No conflict.
        </p>

        <div className="flex flex-wrap gap-4 mb-16">
          <a
            href="https://github.com/Ataraxy-Labs/weave"
            target="_blank"
            rel="noopener noreferrer"
            className="px-6 py-3 bg-white text-black font-semibold rounded-lg hover:bg-gray-200 transition-colors text-sm"
            style={{ fontFamily: "var(--font-heading)" }}
          >
            GitHub
          </a>
          <a
            href="https://ataraxy-labs.com/blogs/entity-level-merge-driver"
            className="px-6 py-3 border border-white/20 rounded-lg hover:border-white/40 transition-colors text-sm"
            style={{ fontFamily: "var(--font-heading)" }}
          >
            Read the Technical Deep Dive
          </a>
          <a
            href="/llms.txt"
            className="px-6 py-3 border border-white/20 rounded-lg hover:border-white/40 transition-colors text-sm"
            style={{ fontFamily: "var(--font-heading)" }}
          >
            llms.txt
          </a>
        </div>

        {/* Install */}
        <div className="mb-16">
          <pre><code>brew install ataraxy-labs/tap/weave</code></pre>
        </div>
      </section>

      {/* Benchmark */}
      <section className="px-6 py-16 border-t border-white/10">
        <div className="max-w-5xl mx-auto">
          <h2 className="text-3xl font-bold mb-8" style={{ fontFamily: "var(--font-heading)" }}>
            Benchmark: 31 Real-World Merge Scenarios
          </h2>
          <div className="grid md:grid-cols-3 gap-8 mb-8">
            <div className="border border-white/10 rounded-lg p-8">
              <p className="text-sm text-gray-500 uppercase tracking-wider mb-2" style={{ fontFamily: "var(--font-heading)" }}>Git merge</p>
              <p className="text-5xl font-bold text-gray-500" style={{ fontFamily: "var(--font-heading)" }}>15/31</p>
              <p className="text-gray-500 mt-2">48% clean</p>
            </div>
            <div className="border border-white/10 rounded-lg p-8">
              <p className="text-sm text-gray-500 uppercase tracking-wider mb-2" style={{ fontFamily: "var(--font-heading)" }}>Mergiraf</p>
              <p className="text-5xl font-bold text-gray-400" style={{ fontFamily: "var(--font-heading)" }}>26/31</p>
              <p className="text-gray-400 mt-2">83% clean</p>
            </div>
            <div className="border border-white/20 rounded-lg p-8 bg-white/[0.02]">
              <p className="text-sm text-gray-400 uppercase tracking-wider mb-2" style={{ fontFamily: "var(--font-heading)" }}>Weave</p>
              <p className="text-5xl font-bold text-white" style={{ fontFamily: "var(--font-heading)" }}>31/31</p>
              <p className="text-gray-300 mt-2">100% clean</p>
            </div>
          </div>
          <p className="text-gray-500 text-sm">
            Scenarios include adjacent function edits, import conflicts, class member additions, renames, and decorator changes across Python, TypeScript, Rust, Go, Java, and C. Mergiraf (v0.16.3) fails on both-add-at-end, insert-in-middle, and decorator conflict scenarios.
          </p>
        </div>
      </section>

      {/* How it works */}
      <section className="px-6 py-16 border-t border-white/10">
        <div className="max-w-5xl mx-auto">
          <h2 className="text-3xl font-bold mb-12" style={{ fontFamily: "var(--font-heading)" }}>
            How It Works
          </h2>
          <div className="grid md:grid-cols-2 gap-10">
            <div>
              <h3 className="text-lg font-semibold mb-3" style={{ fontFamily: "var(--font-heading)" }}>1. Parse with tree-sitter</h3>
              <p className="text-gray-400 leading-relaxed">
                All three versions (base, ours, theirs) are parsed into entity lists: functions, classes, methods, imports, constants, and types. 12 languages supported.
              </p>
            </div>
            <div>
              <h3 className="text-lg font-semibold mb-3" style={{ fontFamily: "var(--font-heading)" }}>2. Match entities</h3>
              <p className="text-gray-400 leading-relaxed">
                Entities are matched across versions by name and structural hash. Renames are detected when an entity disappears but a new one appears with the same AST-normalized hash.
              </p>
            </div>
            <div>
              <h3 className="text-lg font-semibold mb-3" style={{ fontFamily: "var(--font-heading)" }}>3. Classify changes</h3>
              <p className="text-gray-400 leading-relaxed">
                Each entity is categorized: added, deleted, modified in one branch, or modified in both. Class members and imports are treated as unordered sets.
              </p>
            </div>
            <div>
              <h3 className="text-lg font-semibold mb-3" style={{ fontFamily: "var(--font-heading)" }}>4. Merge or flag</h3>
              <p className="text-gray-400 leading-relaxed">
                Non-conflicting changes are applied automatically. Only true semantic conflicts (same entity modified differently in both branches) produce conflict markers.
              </p>
            </div>
          </div>
        </div>
      </section>

      {/* Features */}
      <section className="px-6 py-16 border-t border-white/10">
        <div className="max-w-5xl mx-auto">
          <h2 className="text-3xl font-bold mb-12" style={{ fontFamily: "var(--font-heading)" }}>
            Features
          </h2>
          <div className="grid md:grid-cols-3 gap-8">
            <div className="border border-white/10 rounded-lg p-6">
              <h3 className="text-base font-semibold mb-3" style={{ fontFamily: "var(--font-heading)" }}>12 Languages</h3>
              <p className="text-gray-400 text-sm leading-relaxed">
                TypeScript, TSX, JavaScript, Python, Go, Rust, Java, C, C++, Ruby, C#, Fortran. Each with language-specific entity extraction.
              </p>
            </div>
            <div className="border border-white/10 rounded-lg p-6">
              <h3 className="text-base font-semibold mb-3" style={{ fontFamily: "var(--font-heading)" }}>Rename Detection</h3>
              <p className="text-gray-400 text-sm leading-relaxed">
                Structural hashing detects renamed functions. One branch renames, the other modifies the body -- both changes merge cleanly.
              </p>
            </div>
            <div className="border border-white/10 rounded-lg p-6">
              <h3 className="text-base font-semibold mb-3" style={{ fontFamily: "var(--font-heading)" }}>Commutative Imports</h3>
              <p className="text-gray-400 text-sm leading-relaxed">
                Import statements are merged as sets. Both branches add different imports? Unioned, deduplicated, sorted.
              </p>
            </div>
            <div className="border border-white/10 rounded-lg p-6">
              <h3 className="text-base font-semibold mb-3" style={{ fontFamily: "var(--font-heading)" }}>Unordered Class Members</h3>
              <p className="text-gray-400 text-sm leading-relaxed">
                Methods added at the same position in a class resolve cleanly. Method order within a class rarely carries semantic meaning.
              </p>
            </div>
            <div className="border border-white/10 rounded-lg p-6">
              <h3 className="text-base font-semibold mb-3" style={{ fontFamily: "var(--font-heading)" }}>Inner Entity Merge</h3>
              <p className="text-gray-400 text-sm leading-relaxed">
                When both branches modify the same class, methods are matched by name and merged independently. Different methods, no conflict.
              </p>
            </div>
            <div className="border border-white/10 rounded-lg p-6">
              <h3 className="text-base font-semibold mb-3" style={{ fontFamily: "var(--font-heading)" }}>Comment Bundling</h3>
              <p className="text-gray-400 text-sm leading-relaxed">
                Docstrings, decorators, and annotations are bundled with the entity they describe. They move with the function through merges.
              </p>
            </div>
          </div>
        </div>
      </section>

      {/* MCP Server */}
      <section className="px-6 py-16 border-t border-white/10">
        <div className="max-w-5xl mx-auto">
          <h2 className="text-3xl font-bold mb-4" style={{ fontFamily: "var(--font-heading)" }}>
            MCP Server for Multi-Agent Coordination
          </h2>
          <p className="text-gray-400 mb-8 max-w-3xl">
            Weave includes an MCP server with 15 tools for AI agent integration. Agents can claim entities before editing, detect conflicts, preview merges, analyze dependencies, and get structured conflict summaries. Works with Claude, GPT, Cursor, Windsurf, Zed, and any MCP-compatible tool.
          </p>
          <div className="grid md:grid-cols-3 gap-4 text-sm">
            <div className="border border-white/10 rounded-lg p-4">
              <code className="text-white">weave_extract_entities</code>
              <p className="text-gray-500 mt-1">List all entities in a file</p>
            </div>
            <div className="border border-white/10 rounded-lg p-4">
              <code className="text-white">weave_claim_entity</code>
              <p className="text-gray-500 mt-1">Advisory lock before editing</p>
            </div>
            <div className="border border-white/10 rounded-lg p-4">
              <code className="text-white">weave_release_entity</code>
              <p className="text-gray-500 mt-1">Release lock after editing</p>
            </div>
            <div className="border border-white/10 rounded-lg p-4">
              <code className="text-white">weave_status</code>
              <p className="text-gray-500 mt-1">Entity status with claims</p>
            </div>
            <div className="border border-white/10 rounded-lg p-4">
              <code className="text-white">weave_who_is_editing</code>
              <p className="text-gray-500 mt-1">Check entity edit status</p>
            </div>
            <div className="border border-white/10 rounded-lg p-4">
              <code className="text-white">weave_potential_conflicts</code>
              <p className="text-gray-500 mt-1">Detect multi-agent collisions</p>
            </div>
            <div className="border border-white/10 rounded-lg p-4">
              <code className="text-white">weave_preview_merge</code>
              <p className="text-gray-500 mt-1">Dry-run merge analysis</p>
            </div>
            <div className="border border-white/10 rounded-lg p-4">
              <code className="text-white">weave_agent_register</code>
              <p className="text-gray-500 mt-1">Register agent in state</p>
            </div>
            <div className="border border-white/10 rounded-lg p-4">
              <code className="text-white">weave_agent_heartbeat</code>
              <p className="text-gray-500 mt-1">Keep-alive with work state</p>
            </div>
            <div className="border border-white/10 rounded-lg p-4">
              <code className="text-white">weave_get_dependencies</code>
              <p className="text-gray-500 mt-1">What this entity calls</p>
            </div>
            <div className="border border-white/10 rounded-lg p-4">
              <code className="text-white">weave_get_dependents</code>
              <p className="text-gray-500 mt-1">Who calls this entity</p>
            </div>
            <div className="border border-white/10 rounded-lg p-4">
              <code className="text-white">weave_impact_analysis</code>
              <p className="text-gray-500 mt-1">Transitive blast radius</p>
            </div>
            <div className="border border-white/10 rounded-lg p-4">
              <code className="text-white">weave_diff</code>
              <p className="text-gray-500 mt-1">Entity-level diff between refs</p>
            </div>
            <div className="border border-white/10 rounded-lg p-4">
              <code className="text-white">weave_validate_merge</code>
              <p className="text-gray-500 mt-1">Semantic risk detection</p>
            </div>
            <div className="border border-white/10 rounded-lg p-4">
              <code className="text-white">weave_merge_summary</code>
              <p className="text-gray-500 mt-1">Structured conflict summary</p>
            </div>
          </div>
        </div>
      </section>

      {/* Quick Start */}
      <section className="px-6 py-16 border-t border-white/10">
        <div className="max-w-5xl mx-auto">
          <h2 className="text-3xl font-bold mb-8" style={{ fontFamily: "var(--font-heading)" }}>
            Quick Start
          </h2>
          <div className="space-y-6">
            <div>
              <p className="text-sm text-gray-500 mb-2">Install</p>
              <pre><code>brew install ataraxy-labs/tap/weave</code></pre>
            </div>
            <div>
              <p className="text-sm text-gray-500 mb-2">Set up in your repository</p>
              <pre><code>weave-cli setup</code></pre>
            </div>
            <div>
              <p className="text-sm text-gray-500 mb-2">That&apos;s it. Git merges now use Weave automatically.</p>
              <pre><code>git merge feature-branch</code></pre>
            </div>
            <div>
              <p className="text-sm text-gray-500 mb-2">Preview a merge before running it</p>
              <pre><code>weave-cli preview main feature-branch</code></pre>
            </div>
          </div>
        </div>
      </section>

      {/* Architecture */}
      <section className="px-6 py-16 border-t border-white/10">
        <div className="max-w-5xl mx-auto">
          <h2 className="text-3xl font-bold mb-8" style={{ fontFamily: "var(--font-heading)" }}>
            Architecture
          </h2>
          <pre><code>{`weave/crates/
  weave-core/     Entity extraction, matching, merge algorithm
  weave-driver/   Git merge driver binary
  weave-cli/      Setup, preview, status commands
  weave-crdt/     Automerge-backed coordination state
  weave-mcp/      MCP server (15 tools)`}</code></pre>
          <p className="text-gray-400 mt-4 text-sm">
            Built in Rust. Entity extraction powered by <a href="https://github.com/Ataraxy-Labs/sem" className="text-white underline hover:text-gray-300">sem-core</a> with tree-sitter. CRDT state backed by Automerge. MCP server via rmcp.
          </p>
        </div>
      </section>

      {/* Research */}
      <section className="px-6 py-16 border-t border-white/10">
        <div className="max-w-5xl mx-auto">
          <h2 className="text-3xl font-bold mb-8" style={{ fontFamily: "var(--font-heading)" }}>
            Research
          </h2>
          <p className="text-gray-400 mb-6">
            Weave&apos;s merge algorithm synthesizes ideas from 7 academic papers:
          </p>
          <ul className="space-y-3 text-gray-400 text-sm">
            <li className="border-l-2 border-white/20 pl-4"><strong className="text-white">LastMerge</strong> (arXiv:2507.19687) -- Unordered NonTerminal configuration for class members</li>
            <li className="border-l-2 border-white/20 pl-4"><strong className="text-white">Mergiraf</strong> (v0.16.3) -- Commutative import merge, comment bundling</li>
            <li className="border-l-2 border-white/20 pl-4"><strong className="text-white">ConGra</strong> (arXiv:2409.14121) -- Conflict taxonomy, entity isolation is optimal granularity</li>
            <li className="border-l-2 border-white/20 pl-4"><strong className="text-white">Sesame</strong> (2024) -- Separator preprocessing for whitespace normalization</li>
            <li className="border-l-2 border-white/20 pl-4"><strong className="text-white">RefFilter / IntelliMerge</strong> -- Rename detection via structural hashing</li>
            <li className="border-l-2 border-white/20 pl-4"><strong className="text-white">Unison</strong> -- Content-addressed code, AST-normalized hashing</li>
          </ul>
        </div>
      </section>

      {/* Footer */}
      <footer className="px-6 py-12 border-t border-white/10 text-center text-gray-600 text-sm">
        <p>MIT License. Built by <a href="https://ataraxy-labs.com" className="text-gray-400 hover:text-white transition-colors">Ataraxy Labs</a>.</p>
      </footer>
    </div>
  );
}
