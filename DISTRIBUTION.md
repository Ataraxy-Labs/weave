# Distribution channels

Six channels can put weave on someone's machine. They were built at different
times and are in different states of repair. This file is the inventory: what
each channel is, whether it currently works, and what a release has to touch to
move it forward. Nothing here is deleted on the strength of this audit — a
channel that is dormant is not the same as a channel that is gone, and the cost
of guessing wrong is a broken install for a stranger.

**The one fact that governs all of them:** the private tree's root `Cargo.toml`
carries a `[patch.crates-io]` pointing `sem-core` at an absolute path on one
machine. Anything that builds from a clean checkout of *this* repo fails. The
public tree is produced by the export script, which strips that patch; every
channel below builds from the public tree, not this one.

---

## 1. GitHub Releases — **live, and the source of truth**

`.github/workflows/release.yml`.

Triggered by a push to `main` that changes `crates/weave-core/Cargo.toml`, or
by `workflow_dispatch`. It reads the version out of that one manifest, skips if
the tag already exists, otherwise tags `vX.Y.Z`, cross-builds `weave`,
`weave-driver` and `weave-mcp` for five targets (macOS arm64/x86_64, Linux
arm64/x86_64, Windows x86_64), and publishes tarballs, zips, a `checksums.txt`
and generated release notes.

Every other channel downstream of a release reads from here — npm fetches these
artifacts, Homebrew's core formula is bumped from these tags, and
`docs/changelog.html` now points readers here for anything after v0.2.6.

**A release must:** bump the version in **all six** `crates/*/Cargo.toml` in one
commit. The workflow only *reads* `weave-core`'s; the other five are bumped by
hand and are currently in lockstep at 0.3.6. A crate left behind is published at
its old version by the `publish-crates` job without complaint.

## 2. crates.io — **live**

`publish-crates` in the same workflow. Publishes `weave-core`, `weave-crdt`,
`weave-cli`, `weave-driver`, `weave-mcp` in dependency order with a 30s index
wait between each, treating "already exists" as success and any other failure as
fatal. `weave-github` is `publish = false` and is correctly absent.

**A release must:** nothing beyond the version bump above.

## 3. npm — `@ataraxy-labs/weave` — **BROKEN since 2026-05-15**

`package.json` + `scripts/`.

The design is sound: `postinstall.mjs` resolves the host triple, downloads the
matching release artifacts, verifies them against `checksums.txt`, and unpacks
them into `vendor/`. `package-meta.mjs` and `verify-checksum.mjs` are intact and
correct.

What is missing is the three launcher shims. `package.json` declares

```json
"bin": { "weave": "./bin/weave.js", "weave-driver": "./bin/weave-driver.js", "weave-mcp": "./bin/weave-mcp.js" }
```

and `bin/` does not exist. All three files were deleted as collateral in commit
`2943403` ("remove site directory and junk files", 2026-05-15) alongside a
checked-in Next.js build. An `npm install` of a package published today gets the
binaries into `vendor/` and then has nothing on `PATH` to run them.

This is not caught by CI: the `Check npm package contents` step runs
`npm pack --dry-run`, which does not fail on `files` entries that do not exist.

**To repair:** restore three shims that `spawn` the corresponding
`getInstalledBinaryPath(name)` from `package-meta.mjs` and forward argv and the
exit code (`2943403^:bin/weave.js` has the originals). Then make the CI step
assert the three `bin` targets exist before publishing, so the same deletion
cannot pass silently twice. `package.json`'s own `version` field (0.3.4) is
stale but harmless — `sync-package-version.mjs` rewrites it from the release
version at publish time.

## 4. Homebrew — **two separate things, one live and one stale**

**`brew install weave` (homebrew-core) — live.** The formula the README badge
and `docs/index.html` point at lives in homebrew-core, not in this repo. It is
bumped from the GitHub release tag by Homebrew's own tooling. Nothing in this
repo updates it, and nothing in this repo needs to.

**`Formula/weave.rb` (in-repo) — stale, and would fail if used.** Three
independent problems:

- `url` is pinned to the `v0.1.1` tarball; the current tag is `v0.3.6`.
- `license "MIT"`; every crate is `MIT OR Apache-2.0`.
- the `test do` block runs `#{bin}/weave-cli bench`. There is no `weave-cli`
  binary — `crates/weave-cli` builds a binary named `weave`. `brew test` fails
  on the first line.

Nothing in CI references this file, so none of that has ever been caught. It is
kept because a tap formula is the fallback if the core formula is ever dropped,
and because deleting it would lose the `std_cargo_args` install recipe.

**Before using it:** fix all three, and decide whether it is a source-build tap
formula (what it is now) or a bottle formula pointing at the release artifacts
(what would actually be fast).

## 5. Nix — `flake.nix` / `package.nix` / `shell.nix` — **untested**

`package.nix` reads its version straight out of `crates/weave-cli/Cargo.toml`,
so it does not go stale on a bump. It builds the whole workspace with
`buildRustPackage`, `doCheck = false`, pinned by the committed `Cargo.lock`.
`flake.nix` exposes it as `packages.default` and `shell.nix` as the dev shell.

No CI job evaluates the flake, so "untested" is the honest state: it has not
been shown broken, and it has not been shown to work since the split. The
commented-out `postInstall` block references an `autocompletion/` directory that
does not exist in this repo.

**Before relying on it:** `nix build .#default` against the *public* tree (the
`[patch.crates-io]` makes it fail against this one), and either restore the
completion files or drop the dead block.

## 6. Fly.io — `weave-github` webhook service — **dormant**

`fly.toml` (app `weave-merge`, region `sjc`, scale-to-zero) plus
`crates/weave-github/Dockerfile`.

`weave-github` is `publish = false`, is not built by `ci.yml`, is not built by
`release.yml`, and has no tests. It is the only crate in the workspace with no
gate over it at all. Three specific hazards if it is redeployed:

- `crates/weave-github/fly.toml` is a byte-identical copy of the root
  `fly.toml`, including `dockerfile = "crates/weave-github/Dockerfile"` — a path
  that only resolves from the repo root. The nested copy cannot work from its
  own directory and is a trap for whoever runs `fly deploy` from inside the
  crate.
- the Dockerfile does `COPY . .` then `cargo build --release -p weave-github`,
  which walks straight into the local `[patch.crates-io]`. It builds from the
  public tree and not from this one.
- `.github/dependabot.yml` watches `crates/weave-github` for base-image bumps,
  so the Dockerfile's `FROM` lines stay current whether or not anyone deploys.

**Before redeploying:** delete one of the two `fly.toml` copies, and add
`weave-github` to whatever gate is meant to cover it — right now a change that
breaks it is invisible until deploy time.

---

## Release checklist, as it actually stands

1. Bump the version in all six `crates/*/Cargo.toml` and commit to `main`.
2. `release.yml` does the rest for GitHub Releases and crates.io.
3. Homebrew core follows the tag on its own.
4. npm publishes a package that installs nothing runnable until the `bin/` shims
   are restored (see section 3).
5. `Formula/weave.rb` and the Nix and Fly definitions are not touched by any
   release and do not need to be — none of them is currently wired to one.
