# axum-kickoff

A [cargo-generate](https://github.com/cargo-generate/cargo-generate) template for production-ready Rust web applications built on [Axum](https://github.com/tokio-rs/axum), following best practices from the [crates.io](https://github.com/rust-lang/crates.io) backend implementation.

This repository **is** the template — its sources contain Liquid placeholders and do not compile as-is. Generating a project renders them into a fully working application.

## What you get

- **Modern Stack**: Axum 0.8 with Tokio async runtime
- **Database**: Toasty ORM with SQLite (zero-setup) or PostgreSQL
- **Authentication**: OAuth sign-in (GitHub, Google, Facebook — selectable at generation), session-based auth, and scoped API tokens
- **Frontend**: Server-side rendering with Askama, HTMX, and Alpine.js
- **Security**: Comprehensive middleware (security headers, rate limiting, CSRF, etc.)
- **Testing**: Integration test infrastructure with snapshot testing
- **Storage**: Local filesystem storage (pluggable architecture for future backends)
- **Background Jobs**: Built-in worker for async job processing
- **API Docs**: OpenAPI/Swagger UI out of the box (utoipa)
- **Optional subsystems** (selected at generation time): Prometheus metrics, Sentry error reporting, jemalloc allocator

## Generate a project

```bash
cargo install cargo-generate
cargo generate --git https://github.com/luizvbo/axum-kickoff --name my-app
```

You'll be prompted for the options below (or pass `-d <key>=<value>` to skip prompts):

| Option | Values | Default | Effect |
| ------ | ------ | ------- | ------ |
| `project-name` | any crate-safe name | asked once | Package name, binary name, session cookie (`<crate_name>_session`), default SQLite file (`<crate_name>.db`), PostgreSQL `application_name`, API title, user-agent strings, and the on-page application branding |
| `database` | `sqlite` / `postgresql` | `sqlite` | `sqlite`: zero-setup file database. `postgresql`: adds the Toasty Postgres driver and a `postgresql://` default `DATABASE_URL`. SQLite stays enabled either way — the test suite uses in-memory SQLite regardless |
| `oauth_github` | `true` / `false` | `true` | Compiles in the GitHub OAuth provider; enabled at runtime when `GH_CLIENT_ID`/`GH_CLIENT_SECRET` are set |
| `oauth_google` | `true` / `false` | `false` | Same for Google (`GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET`) |
| `oauth_facebook` | `true` / `false` | `false` | Same for Facebook (`FACEBOOK_CLIENT_ID`/`FACEBOOK_CLIENT_SECRET`) |
| `metrics` | `true` / `false` | `false` | Adds the `prometheus` dependency, a `metrics` feature in `default`, the `/metrics` endpoint, and request instrumentation middleware |
| `sentry` | `true` / `false` | `false` | Adds `sentry` + `sentry-tracing` dependencies and feature, error-event capture via the tracing layer, and `SENTRY_DSN` support |
| `jemalloc` | `true` / `false` | `false` | Adds `tikv-jemallocator` and sets jemalloc as the global allocator |

The background worker, filesystem storage, and Swagger/OpenAPI subsystems are
unconditional parts of the generated application — they are not options.

Non-interactive generation:

```bash
cargo generate --git https://github.com/luizvbo/axum-kickoff \
  --name my-app --silent \
  -d database=postgresql -d metrics=true -d sentry=true -d jemalloc=true
```

> **PostgreSQL note**: the checked-in migrations are written in SQLite dialect
> (`AUTOINCREMENT`). When `database=postgresql` is selected the Postgres driver
> is compiled in and `DATABASE_URL` defaults to Postgres, but the migration SQL
> must be adapted for PostgreSQL before `migrate` will run against a real
> Postgres database. The test suite is unaffected — it runs on in-memory
> SQLite.

## After generation

```bash
cd my-app
just setup        # npm install + vendor JS libraries (HTMX, Alpine.js)
cp .env.sample .env  # then edit; required: SESSION_KEY, WEB_ALLOWED_ORIGINS,
                     # and credentials for each OAuth provider you enable
cargo run -- server  # http://localhost:8888
```

## Documentation

The generated project ships its own docs under `docs/` — start with
[Getting Started](docs/GETTING_STARTED.md) and the
[Configuration reference](docs/CONFIGURATION.md). They are Liquid-rendered at
generation time and describe your project, not this repository.

## Working on this template

The checkout does not compile — `Cargo.toml` and several `src/` files contain
Liquid placeholders and conditionals. Verification happens on rendered output:
edit source → render → check the rendered project. `.github/workflows/template-ci.yml`
runs this over the option matrix and is the source of truth.

The README split: `README.md` (this file) is the repository's own readme and is
`ignore`d by cargo-generate. The readme shipped into generated projects lives in
`{{"README"}}.md` — cargo-generate renders Liquid in filenames too, so it lands
as `README.md` in the output.

See [docs/TEMPLATE.md](docs/TEMPLATE.md) for the full authoring reference
(placeholders, tag-placement rules, conditional files) and the "Working on this
template" section of [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for the
day-to-day workflow.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Acknowledgments

- Inspired by the [crates.io](https://github.com/rust-lang/crates.io) backend implementation
- Built with [Axum](https://github.com/tokio-rs/axum) and [Tokio](https://tokio.rs)
- Uses [Toasty](https://github.com/stepchowfun/toasty) for database ORM
- Frontend powered by [HTMX](https://htmx.org) and [Alpine.js](https://alpinejs.dev)
