# LumaForge

LumaForge is a desktop application that turns ComfyUI workflows into ready-to-use remote GPU workspaces. It provisions GPU infrastructure, prepares runtime environments, and executes workflows so users do not have to configure remote machines manually.

## Frontend

At the current stage, the frontend under [`src`](./src) is a development console for interacting with the Rust API exposed by the native backend.

## Native Backend

The Rust/Tauri native backend owns application workflows, local SQLite persistence, secure credential storage, authoritative validation, and provider integrations. It exposes UI-safe commands and events to the frontend. See [src-tauri/README.md](./src-tauri/README.md) for architecture and development notes.

## Bundled Catalog

The catalog under [`bundled/catalog`](./bundled/catalog) is packaged with the desktop app. Its revisioned entries define workflows, including metadata, ComfyUI graphs, model assets, and execution contracts, as well as execution schemas, runtime presets, and digest-pinned worker runtime contracts. Each reference pins an exact catalog contract, ID, and revision; release tooling creates new revisions instead of modifying existing ones.

## Workers

The [Provisioner Worker](./workers/provisioner/README.md) is a one-shot container service that prepares the mounted ComfyUI workspace and downloads its required model assets.

The [RunPod Endpoint Worker](./workers/runpod-endpoint/README.md) is a RunPod Serverless handler that executes the workflow baked into its image and returns UI-safe references to generated artifacts.

### Deploy Provisioner

[Deploy Provisioner](./.github/workflows/deploy-provisioner.yml) releases the one-shot Provisioner Worker image. The workflow calculates the next `provisioner` runtime contract revision and creates a new revision for each catalog workflow whose latest revision references that contract.

### Deploy RunPod Endpoint

[Deploy RunPod Endpoint](./.github/workflows/deploy-runpod-endpoint.yml) releases a RunPod Endpoint Worker image for one existing workflow revision. GitHub Action accepts:

- `workflow_id`: workflow ID under `bundled/catalog/entries/workflows`; defaults to `comfyui-hidream-o1-dev`.
- `workflow_revision`: existing source revision to bake into the image; defaults to `1.0.0`.

## Development

Run all commands from the repository root.

### Setup & Build

| Command               | Purpose                                     |
| --------------------- | ------------------------------------------- |
| `bun install`         | Install JavaScript dependencies.            |
| `bun run dev`         | Start the Vite frontend development server. |
| `bun run tauri dev`   | Run the Tauri desktop application.          |
| `bun run build`       | Build and type-check the frontend.          |
| `bun run tauri build` | Build the desktop application bundle.       |

### Lint & Formatting

| Command                                                                                         | Purpose                                  |
| ----------------------------------------------------------------------------------------------- | ---------------------------------------- |
| `bun run lint`                                                                                  | Run ESLint.                              |
| `bun run lint:fix`                                                                              | Apply ESLint autofixes.                  |
| `bun run format`                                                                                | Format frontend files with ESLint fixes. |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check`                                        | Check native backend formatting.         |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | Run strict native backend linting.       |

### Tests

| Command                                                                                                | Purpose                           |
| ------------------------------------------------------------------------------------------------------ | --------------------------------- |
| `cargo test --manifest-path src-tauri/Cargo.toml`                                                      | Run native backend tests.         |
| `PYTHONPATH=workers/provisioner/src python3 -m unittest discover -s workers/provisioner/tests`         | Run provisioner worker tests.     |
| `PYTHONPATH=workers/runpod-endpoint/src python3 -m unittest discover -s workers/runpod-endpoint/tests` | Run RunPod endpoint worker tests. |

### Codegen

Files under [`src/generated`](./src/generated) are generated and must not be edited manually.

| Command                    | Purpose                                                                               |
| -------------------------- | ------------------------------------------------------------------------------------- |
| `bun run codegen:commands` | Regenerate `src/generated/commands.ts` after Tauri command or event contract changes. |
