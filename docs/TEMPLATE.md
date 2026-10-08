# Working on the axum-kickoff template

This document describes how this repository works *as a cargo-generate
template*. It is excluded from generated projects (via `ignore` in
`cargo-generate.toml`) — if you are looking at a generated project's checkout,
this file does not exist and none of it applies.

## The source tree does not compile — by design

`Cargo.toml` and several `src/` files contain Liquid placeholders
(`{{project-name}}`, `{{crate_name}}`, `{% if metrics %}` …). `cargo`/`rustfmt`
cannot parse the checkout, and that is expected. **All verification happens on
rendered output.** The template CI
(`.github/workflows/template-ci.yml`, also excluded from generated projects)
renders a matrix of option combinations into `$RUNNER_TEMP` and runs
`cargo check` / `cargo clippy --all-targets` / `cargo fmt --check` / `cargo test`
there. It is the source of truth for "the template works".

Workflow:

1. Edit the template source.
2. Render: `cargo generate --path . --name test-app --destination /tmp/render --vcs none --silent` (plus `-d key=value` to override placeholder defaults).
3. Run the cargo checks inside `/tmp/render/test-app`.

## The exclude boundary

Liquid's `{{ }}` / `{% %}` delimiters are **not** configurable in
cargo-generate, and several file families collide with them:

- `templates/` — Askama uses the same `{{ }}`/`{% %}` syntax. `{{ ctx.x }}`
  would render silently empty; `{% block %}`/`{% extends %}` are hard parse
  errors.
- `.github/` — GitHub Actions `${{ github.* }}` expressions collide.
- `static/` — vendored JS bundles contain `{{ }}` sequences.
- Docs that show literal Askama syntax (`ADD_HTMX_FORM.md`,
  `ADD_NEW_PAGE.md`, `ADD_PROTECTED_ROUTE.md`, `CSRF_PROTECTION.md`,
  `HTMX_ASKAMA_PATTERNS.md`).

Anything in `exclude` is copied **byte-for-byte** — no substitutions, not even
in file names. The rule: **process nothing that doesn't need a substitution.**
Adding a file to `exclude` is the mechanism; `{% raw %}`/`{% endraw %}` is only
acceptable inside a file that genuinely *needs* Liquid processing (currently a
single justfile recipe using just's own `{{ }}` interpolation).

`ignore` entries are different: matching files are **not copied** to the
generated project at all. Matching is literal (no globs), evaluated against
source paths before file-name substitution.

## Where names and options get substituted

`{{project-name}}` (kebab-case as typed) lands in `Cargo.toml`
`[package].name`/`[[bin]].name`/`default-run`, the README, user-agent strings,
and the OpenAPI title. `{{crate_name}}` (the snake_case form) lands in `use`
paths, the session cookie name (`<crate_name>_session` in
`src/middleware/session.rs`), the default SQLite file name, the Postgres
`application_name`, and `src/tests/snapshots/` **file names** — insta derives
snapshot names from the crate name, so a literal `{{crate_name}}__...snap`
filename is renamed during generation.

`{% if metrics %}` / `{% if sentry %}` / `{% if jemalloc %}` gate the matching
`Cargo.toml` feature+dependency entries **and** the `#[cfg(feature = "...")]`
code. Both must be removed together: a `#[cfg(feature = "x")]` for an
undeclared feature trips `unexpected_cfgs` (which fails `-D warnings`). The
metrics-only files (`src/metrics.rs`, `src/middleware/metrics.rs`,
`src/tests/metrics.rs`, the metrics snapshot) are dropped by a
`[conditional.'!metrics']` ignore.

`{% if oauth_github %}` / `{% if oauth_google %}` / `{% if oauth_facebook %}`
gate the provider entries in `src/oauth.rs`, the credential blocks in
`.env.sample`, and the provider docs. Unlike the feature flags, these only
control which `OAuthProviderSpec`s are *compiled in* — the generated project
still enables each provider at runtime via its `*_CLIENT_ID`/`*_CLIENT_SECRET`
pair (setting only one is a startup error). The login page lists enabled
providers at runtime from `PageContext.oauth_providers` — no Liquid is needed
in `templates/`.

### Tag placement and whitespace

Liquid tags render in place. To keep `cargo fmt --check` happy on **both**
branches, tags hug surrounding lines:

```rust
{% if metrics %}#[cfg(feature = "metrics")]
pub mod metrics;
{% endif %}pub mod middleware;
```

`{% if %}` sits at column 0 before the conditional block's first line and
`{% endif %}` is glued to the following line (with its indentation). Rendering
`metrics=false` joins the neighbours cleanly; `metrics=true` reproduces the
original file. For `#[cfg(not(feature = "x"))]` attributes, conditionalize only
the attribute line — the item itself is needed in both renders.

One known cosmetic artifact: `{% if %}`/`{% endif %}` lines left between items
emit a single blank line when false — fine between items/statements, but never
leave a conditional at EOF or immediately after a `{` (rustfmt strips blanks
there and `fmt --check` diffs).

## Runtime branding

`templates/` never sees Liquid, so `templates/index.html` branding is resolved
at **runtime**: `PageContext.app_name` (in `src/router.rs`) derives a
title-cased name from `env!("CARGO_PKG_NAME")`, and templates use
`{{ ctx.app_name }}`. Any `PageContext` literal (error renderer, tests) must
populate the field.

## CI split

- `.github/workflows/ci.yml` — the **generated project's** CI (fmt, clippy,
  doc, tests, cargo-deny/machete/audit, zizmor). It is copied verbatim
  (`exclude`), so in the template repository it must not run: a `detect` job
  checks for `cargo-generate.toml` (which is `ignore`d from renders) and every
  cargo job is gated on its absence.
- `.github/workflows/template-ci.yml` — the **template's** CI. Renders the
  option matrix outside the checkout dir and runs cargo checks/tests there;
  `ignore`d from renders.

cargo-deny, cargo-machete and cargo-audit run against a rendered manifest in
`template-ci.yml` — they cannot parse the Liquid-annotated `Cargo.toml` in
this checkout.

## Follow-ups not yet done

- worker, storage and swagger/utoipa remain unconditional subsystems — they
  have no Cargo features today, so there is nothing to conditionally render.
  Feature-gate them first before offering include/exclude options.
- `migrations/` SQL is SQLite-dialect (`AUTOINCREMENT`); a `postgresql`
  render compiles and tests fine (tests use in-memory SQLite), but the
  migration SQL needs a Postgres translation before `migrate` works against a
  real Postgres server.
