# Documentation Index

This is the recommended starting point for the `axum-kickoff` documentation. The docs below are organized as a learning path rather than an alphabetical list.

## Recommended reading order

1. **[README.md](../README.md)** — Project overview, feature summary, and quick start.
2. **[GETTING_STARTED.md](GETTING_STARTED.md)** — Clone, configure, and run the application locally.
3. **[ARCHITECTURE.md](ARCHITECTURE.md)** — High-level design, request flow, and component map.
4. **[HOW_TO_GUIDES.md](HOW_TO_GUIDES.md)** — Practical recipes for common tasks.
   - [Add a New Page](ADD_NEW_PAGE.md)
   - [Add a New Model](ADD_NEW_MODEL.md)
   - [Add a Protected Route](ADD_PROTECTED_ROUTE.md)
   - [Add an HTMX Form](ADD_HTMX_FORM.md)
5. **[CONFIGURATION.md](CONFIGURATION.md)** — Complete environment variable reference.
6. **[TESTING.md](TESTING.md)** — Testing conventions and the `TestApp` harness.
7. **[DEVELOPMENT.md](DEVELOPMENT.md)** — Local workflow, conventions, and contribution tips.

## Core concepts

- **[Authentication](AUTHENTICATION.md)** — GitHub OAuth, session cookies, and scoped API tokens.
- **[Authorization with Token Scopes](api-token-scopes.md)** — How `ActionScope`/`AuthCheck` work.
- **[CSRF Protection](CSRF_PROTECTION.md)** — Per-session CSRF tokens and HTMX integration.
- **[Rate Limiting](RATE_LIMITING.md)** — Token-bucket rate limiting and configuration.
- **[Middleware Stack](MIDDLEWARE.md)** — Middleware ordering and security features.
- **[Storage](STORAGE.md)** — File storage abstraction and local filesystem backend.
- **[HTMX + Askama Patterns](HTMX_ASKAMA_PATTERNS.md)** — Server-rendered interactive UI patterns.
- **[Error Handling](DEVELOPMENT.md#error-handling)** — `AppResult`, `AppError`, and response formatting.

## Operations

- **[Deployment](DEPLOYMENT.md)** — Docker, systemd, Nginx, and cloud deployment options.
- **[Production Checklist](PRODUCTION_CHECKLIST.md)** — Checklist before going live.
- **[Quickwit Integration](quickwit-integration.md)** — Centralized log analytics setup.
- **[Rate Limiting Redis Upgrade](rate-limiting-redis-upgrade.md)** — Optional migration to Redis (not implemented by default).

## Implementation plans (internal)

These documents are kept for reference but are not part of the end-user docs:

- **[Crates.io Best-Practices Implementation Plan](local/CRATES_IO_IMPLEMENTATION_PLAN.md)** — Internal task prompts for aligning the template with crates.io patterns.
- **[Optional Add-ons](local/OPTIONAL_ADDONS.md)** — Feature prompts that can be added when a concrete application needs them.
- **[Roadmap](ROADMAP.md)** — Longer-term feature plans and design priorities.
