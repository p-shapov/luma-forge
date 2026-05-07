# Luma Forge

Tauri, React, TanStack Router, shadcn/ui.

## Code Generation

Generated files live in `src/generated` and should not be edited manually.

- `bun run codegen` regenerates all generated frontend contracts.
- `bun run codegen:routes` regenerates `src/generated/routeTree.gen.ts` with TanStack Router CLI.
- `bun run codegen:routes:watch` watches `src/routes` and regenerates the route tree on changes.
- `bun run codegen:commands` regenerates `src/generated/commands.ts` from Tauri commands via `tauri-specta`.

TanStack Router also regenerates the route tree during Vite dev/build through the Vite plugin. Use the explicit scripts when you need a clean one-shot regeneration without starting the app.

## Development

- `bun run dev` starts the Vite frontend.
- `bun run build` builds and type-checks the frontend.
- `bun run lint` runs ESLint.
- `bun run format` formats through ESLint autofix.
- `bun run tauri` runs the Tauri CLI.
