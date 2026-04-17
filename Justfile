# rele — project-wide task runner
# Run `just --list` for available recipes.

set dotenv-load := false

# Lean toolchain version (must match spec/lean/lean-toolchain)
lean_version := "v4.29.1"

# Quint version to install via npm
quint_version := "0.23.0"

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

# Install all external tooling (Lean + Quint)
install-tools: install-lean install-quint

# ─── Full spec pipeline ──────────────────────────────────────────────────────

# Build Lean oracle + run all spec tests + Quint checks
spec-all: lean-build spec-test quint-check
