# rele — project-wide task runner
# Run `just --list` for available recipes.

set dotenv-load := false

# Lean toolchain version (must match spec/lean/lean-toolchain)
lean_version := "v4.29.1"

# Quint version to install via npm
quint_version := "0.23.0"

# Apalache version to install (JVM-based; no Homebrew formula, falls back to tarball)
apalache_version := "0.56.1"

# ─── Default ──────────────────────────────────────────────────────────────────

# Show available recipes
default:
    @just --list

# ─── Rust ─────────────────────────────────────────────────────────────────────

# Build all workspace crates
build:
    cargo build --workspace

# Run all workspace tests
test:
    cargo test --workspace

# Run clippy on the entire workspace
lint:
    cargo clippy --workspace -- -D warnings

# Check formatting
fmt-check:
    cargo fmt --all --check

# Format all Rust code
fmt:
    cargo fmt --all

# Full CI check: fmt + clippy + test
ci: fmt-check lint test

# ─── Elisp crate ──────────────────────────────────────────────────────────────

# Build the elisp crate
elisp-build:
    cargo build -p rele-elisp

# Test the elisp crate
elisp-test:
    cargo test -p rele-elisp

# Build elisp with JIT support
elisp-build-jit:
    cargo build -p rele-elisp --features jit

# Check elisp with JIT support
elisp-check-jit:
    cargo check -p rele-elisp --features jit

# ─── Spec tests (Rust harness) ────────────────────────────────────────────────

# Build the spec test crate
spec-build:
    cargo build -p rele-elisp-spec-tests

# Run spec tests (differential + JIT trace replay)
spec-test:
    cargo test -p rele-elisp-spec-tests

# Run only JIT trace replay tests
spec-test-jit:
    cargo test -p rele-elisp-spec-tests --test jit_traces

# Run only differential tests (requires Lean oracle binary)
spec-test-diff:
    cargo test -p rele-elisp-spec-tests --test differential

# ─── Lean oracle ──────────────────────────────────────────────────────────────

# Build the Lean oracle binary
lean-build:
    cd spec/lean && lake build

# Clean Lean build artifacts
lean-clean:
    cd spec/lean && lake clean

# Check that the Lean oracle binary exists
lean-check:
    @test -f spec/lean/.lake/build/bin/elisp-oracle \
        && echo "oracle binary: OK" \
        || echo "oracle binary: NOT FOUND — run 'just lean-build'"

# ─── Quint model ──────────────────────────────────────────────────────────────

# Run Quint simulation checking safeExecution invariant
quint-check-safe steps="50":
    quint run --invariant safeExecution --max-steps {{steps}} spec/quint/jit_runtime.qnt

# Run Quint simulation checking noStaleKeepsRunning invariant
quint-check-stale steps="50":
    quint run --invariant noStaleKeepsRunning --max-steps {{steps}} spec/quint/jit_runtime.qnt

# Run both Quint invariant checks
quint-check: quint-check-safe quint-check-stale

# Verify with Apalache (exhaustive, requires Apalache installed)
quint-verify:
    quint verify --invariant safeExecution spec/quint/jit_runtime.qnt
    quint verify --invariant noStaleKeepsRunning spec/quint/jit_runtime.qnt

# Regenerate an ITF trace under spec/quint/traces/<name>.itf.json (example: just quint-trace deopt_cycle 42 80)
quint-trace name seed="1" max-steps="40":
    mkdir -p spec/quint/traces
    quint run --seed={{seed}} --max-steps {{max-steps}} --out-itf spec/quint/traces/{{name}}.itf.json spec/quint/jit_runtime.qnt

# ─── Install tooling ─────────────────────────────────────────────────────────

# Install elan (Lean version manager) and the project's Lean toolchain
install-lean:
    #!/usr/bin/env bash
    set -euo pipefail
    if command -v elan &>/dev/null; then
        echo "elan already installed: $(elan --version)"
    else
        echo "Installing elan..."
        curl -sSf https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh | sh -s -- -y --default-toolchain none
    fi
    # Ensure elan is on PATH for the rest of this script
    export PATH="$HOME/.elan/bin:$PATH"
    echo "elan: $(elan --version)"
    echo "Installing Lean toolchain {{lean_version}}..."
    elan toolchain install leanprover/lean4:{{lean_version}}
    echo "Lean $(lean --version) ready."
    echo ""
    echo "To use lean/lake in your shell, add to your profile:"
    echo '  export PATH="$HOME/.elan/bin:$PATH"'

# Install Quint via npm
install-quint:
    #!/usr/bin/env bash
    set -euo pipefail
    if command -v quint &>/dev/null; then
        echo "quint already installed: $(quint --version)"
    else
        echo "Installing quint@{{quint_version}}..."
        npm install -g @informalsystems/quint@{{quint_version}}
        echo "quint $(quint --version) ready."
    fi

# Install Apalache (TLA+ model checker used by `quint verify`); uses Homebrew if a formula is available, else the pinned GitHub release tarball
install-apalache:
    #!/usr/bin/env bash
    set -euo pipefail
    if command -v apalache-mc &>/dev/null; then
        echo "apalache-mc already installed: $(apalache-mc version 2>&1 | head -1)"
        exit 0
    fi
    # Apalache needs a JVM on PATH.
    if ! command -v java &>/dev/null; then
        echo "java not found — install a JDK first (e.g. 'brew install --cask temurin')." >&2
        exit 1
    fi
    # Prefer Homebrew if a formula/cask ever lands.
    if command -v brew &>/dev/null && brew info apalache &>/dev/null; then
        echo "Installing apalache via Homebrew..."
        brew install apalache
        echo "apalache-mc $(apalache-mc version 2>&1 | head -1) ready."
        exit 0
    fi
    # Fallback: download the pinned release tarball.
    install_dir="$HOME/.apalache"
    mkdir -p "$install_dir"
    url="https://github.com/apalache-mc/apalache/releases/download/v{{apalache_version}}/apalache-{{apalache_version}}.tgz"
    echo "Downloading $url ..."
    tmp=$(mktemp -d)
    curl -sSfL "$url" -o "$tmp/apalache.tgz"
    tar -xzf "$tmp/apalache.tgz" -C "$install_dir"
    rm -rf "$tmp"
    # The tarball extracts to apalache-<version>/; symlink 'current' for stable paths.
    ln -sfn "$install_dir/apalache-{{apalache_version}}" "$install_dir/current"
    echo "apalache-mc v{{apalache_version}} installed to $install_dir/current"
    echo ""
    echo "To use apalache-mc in your shell, add to your profile:"
    echo '  export PATH="$HOME/.apalache/current/bin:$PATH"'

# Install all external tooling (Lean + Quint + Apalache)
install-tools: install-lean install-quint install-apalache

# ─── Full spec pipeline ──────────────────────────────────────────────────────

# Build Lean oracle + run all spec tests + Quint checks
spec-all: lean-build spec-test quint-check
