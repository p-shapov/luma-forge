# AGENTS.md

## Frontend Scope

- This directory contains the React frontend: app providers, routes, pages, shared UI primitives, generic utilities, and generated frontend contracts.
- Keep React as a thin client over Native-owned state. React may collect input, render status, keep temporary UI state, and call generated Native commands.
- Do not encode durable workflow decisions, provider resource decisions, persistence rules, or secret handling in React.
- Treat Native command responses as authoritative for persisted state and long-running operations.

---

## Structure

- `app/`: React app composition and providers.
- `routes/`: TanStack Router route definitions.
- `pages/`: Page-level UI.
- `shared/`: UI primitives, generic utilities, generated command access, and non-domain helpers only.
- `generated/`: Generated route tree and Native command bindings. Do not edit manually.

---

## Generated Files

- Do not manually edit `generated/**`.

---

## Verification

For frontend changes, run:

- `bun run build`
- `bun run lint`
