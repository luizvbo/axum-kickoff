# Justfile for axum-kickoff
# 
# Install just: cargo install just
# Run commands: just <command>
# List all commands: just --list

# Default recipe - show available commands
default:
    @just --list

# ============================================================================
# Development Commands
# ============================================================================

[doc("Run the development server")]
run:
    cargo run --bin server

[doc("Run the development server with auto-reload (requires cargo-watch)")]
dev:
    cargo watch -x 'run --bin server'

[doc("Run tests")]
test:
    cargo test

[doc("Run tests with nextest faster test runner (requires cargo-nextest)")]
test-nextest:
    cargo nextest run

[doc("Run tests with nextest and accept updated snapshots")]
test-nextest-accept:
    # Accept all snapshot changes without prompting. Use this when you've
    # intentionally changed API responses and want to update the snapshots.
    cargo nextest run --accept

[doc("Run tests with nextest and review snapshot changes")]
test-nextest-review:
    # Review snapshot changes interactively. Use this to inspect what changed
    # before accepting. You'll be prompted to accept or reject each change.
    cargo nextest run --review

[doc("Run tests with output")]
test-verbose:
    cargo test -- --nocapture

# ============================================================================
# Database Commands
# ============================================================================

[doc("Generate a new migration based on model changes")]
migration-generate:
    cargo run --bin cli -- migration generate

[doc("Apply pending migrations to the database")]
migration-apply:
    cargo run --bin cli -- migration apply

[doc("Create a schema snapshot for future migration generation")]
migration-snapshot:
    cargo run --bin cli -- migration snapshot

[doc("Drop the last migration file")]
migration-drop:
    cargo run --bin cli -- migration drop

[doc("Reset the database - WARNING: This will delete all data")]
migration-reset:
    cargo run --bin cli -- migration reset

[doc("Inspect the current database schema as SQL")]
migration-inspect:
    cargo run --bin cli -- migration inspect

# ============================================================================
# Code Quality Commands
# ============================================================================

[doc("Format code with rustfmt")]
fmt:
    cargo fmt

[doc("Check code formatting without making changes")]
fmt-check:
    cargo fmt -- --check

[doc("Run clippy linter")]
clippy:
    cargo clippy --all-targets --all-features

[doc("Run clippy with all warnings treated as errors")]
clippy-strict:
    cargo clippy --all-targets --all-features -- -D warnings

[doc("Run all code quality checks (fmt, clippy, tests)")]
check: fmt-check clippy test

# ============================================================================
# Build Commands
# ============================================================================

[doc("Build the project in debug mode")]
build:
    cargo build

[doc("Build the project in release mode")]
build-release:
    cargo build --release

[doc("Build all binaries")]
build-all:
    cargo build --bins

# ============================================================================
# Documentation Commands
# ============================================================================

[doc("Generate and open documentation")]
docs:
    cargo doc --open

[doc("Generate documentation for all dependencies")]
docs-all:
    cargo doc --document-private-items --open

# ============================================================================
# Cleanup Commands
# ============================================================================

[doc("Clean build artifacts")]
clean:
    cargo clean

[doc("Clean and rebuild")]
clean-build:
    cargo clean && cargo build

[doc("Remove the SQLite database file - WARNING: This will delete all data")]
clean-db:
    rm -f axum_kickoff.db

[doc("Full cleanup (build artifacts + database)")]
clean-all: clean clean-db

# ============================================================================
# Utility Commands
# ============================================================================

[doc("Update all dependencies")]
update:
    cargo update

[doc("Check for outdated dependencies (requires cargo-outdated)")]
outdated:
    cargo outdated

[doc("Generate a new dependency graph (requires cargo-graph)")]
graph:
    cargo graph | dot -Tpng > dependency-graph.png

[doc("Show project size (requires cargo-bloat)")]
bloat:
    cargo bloat --release

[doc("Run cargo audit to check for security vulnerabilities (requires cargo-audit)")]
audit:
    cargo audit
