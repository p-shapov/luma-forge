# LumaForge

LumaForge is a desktop application for preparing remote GPU infrastructure and running ComfyUI workflows on it.

The main goal of the product is to turn a workflow choice into a ready-to-use workspace and then use that workspace to execute the selected ComfyUI workflow on the configured runtime environment.

## Core Concepts

A `Workspace` is a user-created working area for one selected ComfyUI workflow preset. It connects the workflow choice with runtime configuration and its lifecycle state. Users can reuse a workspace while its runtime is available, then clean it up when they no longer need it.

A `WorkspaceRuntime` is the execution environment behind a workspace. It owns runtime-specific setup, execution, and cleanup details so the UI can work with workspace status instead of runtime internals. The current implementation is RunPod; future implementations may run locally or use other providers.

A `WorkflowPreset` is a reusable ComfyUI workflow definition available in the app. It describes what can be run, which runtime image and model assets it needs, and which user inputs are mapped into the ComfyUI graph.

## Development

Native backend architecture and extension notes live in [src-tauri/README.md](./src-tauri/README.md).

## Support Files

The native app keeps support files under the Tauri `app_data_dir()`. On macOS this is:

```text
~/Library/Application Support/<bundle identifier>/
```

Current native support files:

- `native.sqlite`: native SQLite database.
- `logs/`: native diagnostics logs, including `luma-forge.log.YYYY-MM-DD`.

| Command                                                                                                | Purpose                                     |
| ------------------------------------------------------------------------------------------------------ | ------------------------------------------- |
| `bun install`                                                                                          | Install frontend dependencies.              |
| `bun run dev`                                                                                          | Start the Vite frontend development server. |
| `bun run tauri dev`                                                                                    | Run the Tauri desktop application.          |
| `bun run build`                                                                                        | Build and type-check the frontend.          |
| `bun run lint`                                                                                         | Run ESLint.                                 |
| `bun run lint:fix`                                                                                     | Apply ESLint autofixes.                     |
| `bun run format`                                                                                       | Format frontend files with ESLint fixes.    |
| `cargo test --manifest-path src-tauri/Cargo.toml`                                                      | Run active native shell tests.              |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check`                                               | Check active native shell formatting.       |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings`        | Run strict active native shell linting.     |
| `PYTHONPATH=workers/provisioner/src python3 -m unittest discover -s workers/provisioner/tests`         | Run provisioner worker tests.               |
| `PYTHONPATH=workers/runpod-endpoint/src python3 -m unittest discover -s workers/runpod-endpoint/tests` | Run RunPod endpoint worker tests.           |

## Code Generation

Generated files live in `src/generated` and should not be edited manually.

| Command                        | Purpose                                                                                   |
| ------------------------------ | ----------------------------------------------------------------------------------------- |
| `bun run codegen`              | Regenerate all generated frontend contracts.                                              |
| `bun run codegen:routes`       | Regenerate `src/generated/routeTree.gen.ts` after `src/routes/**` changes.                |
| `bun run codegen:routes:watch` | Watch `src/routes` and regenerate the route tree on changes.                              |
| `bun run codegen:commands`     | Regenerate `src/generated/commands.ts` after active Tauri shell command contract changes. |
