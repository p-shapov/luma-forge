# LumaForge

LumaForge is a desktop application that turns ComfyUI workflows into ready-to-use remote GPU workspaces.

Its main goal is to automate the setup of GPU infrastructure, runtime environment, and workflow execution, so users can run selected ComfyUI workflows without manually provisioning or configuring the workspace.

## Core Concepts

A `Workspace` is a persisted working area pinned to one exact bundled workflow revision. A workspace may have one attached runtime at a time; users can provision it, reuse it while ready, and clean up the runtime without deleting the workspace.

A `Runtime` is the remote execution environment attached to a workspace. It has provider-neutral lifecycle state and provider-specific configuration. The current provider is RunPod; provider resource identifiers stay inside the native backend and are not exposed to the UI.

A `RuntimeOperation` is the durable record of a background provision or cleanup operation. It keeps status, provider-specific progress, timestamps, and optional trace correlation even after cleanup removes the runtime.

The bundled catalog is the revisioned source of workflow metadata, ComfyUI graphs, model assets, execution contracts and schemas, runtime presets, and worker image contracts. Catalog references always pin an immutable `(id, revision)` pair.

## Development

Native backend architecture and extension notes live in [src-tauri/README.md](./src-tauri/README.md). Worker-specific setup and contracts live in [workers/provisioner/README.md](./workers/provisioner/README.md) and [workers/runpod-endpoint/README.md](./workers/runpod-endpoint/README.md).

## Support Files

The native app keeps support files under the Tauri `app_data_dir()`. On macOS this is:

```text
~/Library/Application Support/<bundle identifier>/
```

Current native support files:

- `db.sqlite`: native SQLite database.
- `diagnostics.log`: native diagnostics log.

| Command                                                                                                | Purpose                                     |
| ------------------------------------------------------------------------------------------------------ | ------------------------------------------- |
| `bun install`                                                                                          | Install frontend dependencies.              |
| `bun run dev`                                                                                          | Start the Vite frontend development server. |
| `bun run tauri dev`                                                                                    | Run the Tauri desktop application.          |
| `bun run build`                                                                                        | Build and type-check the frontend.          |
| `bun run lint`                                                                                         | Run ESLint.                                 |
| `bun run lint:fix`                                                                                     | Apply ESLint autofixes.                     |
| `bun run format`                                                                                       | Format frontend files with ESLint fixes.    |
| `cargo test --manifest-path src-tauri/Cargo.toml`                                                      | Run native backend tests.                   |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check`                                               | Check native backend formatting.            |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`        | Run strict native backend linting.          |
| `PYTHONPATH=workers/provisioner/src python3 -m unittest discover -s workers/provisioner/tests`         | Run provisioner worker tests.               |
| `PYTHONPATH=workers/runpod-endpoint/src python3 -m unittest discover -s workers/runpod-endpoint/tests` | Run RunPod endpoint worker tests.           |

## Code Generation

Generated files live in `src/generated` and should not be edited manually.

| Command                        | Purpose                                                                               |
| ------------------------------ | ------------------------------------------------------------------------------------- |
| `bun run codegen`              | Regenerate all generated frontend contracts.                                          |
| `bun run codegen:routes`       | Regenerate `src/generated/routeTree.gen.ts` after `src/routes/**` changes.            |
| `bun run codegen:routes:watch` | Watch `src/routes` and regenerate the route tree on changes.                          |
| `bun run codegen:commands`     | Regenerate `src/generated/commands.ts` after Tauri command or event contract changes. |
