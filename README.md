# LumaForge

LumaForge is a macOS desktop application for preparing remote GPU infrastructure and running ComfyUI workflows on it.

The main goal of the product is to turn a local workflow choice into a ready-to-use remote workspace and then use that workspace to execute the selected ComfyUI workflow on remote GPU infrastructure.

## Roadmap

This roadmap is a living document. It captures the current implementation status and known v1 direction, but unchecked items are not a final execution plan. Further steps after the listed v1 work are not defined yet and should be clarified through specs before implementation.

- [x] **Provider Setup**: native-owned provider setup that validates and stores a provider-scoped API key in the secure keyring, with RunPod as the only v1 provider.
- [x] **Workspace Setup**: native-owned creation of a local `Draft` Workspace from a bundled Workflow Preset and Placement Plan without creating provider resources.
- [x] **Provisioner Worker**: container-side worker that prepares the mounted ComfyUI workspace and reports UI-safe provisioning progress.
- [x] **RunPod Endpoint Worker**: define and implement the RunPod Serverless runtime contract between the Serverless Endpoint and the prepared ComfyUI environment.
- [x] **Workspace Provisioning Flow**: native sync loop that creates RunPod resources, invokes the Provisioner Worker, creates the Serverless Endpoint, supports cancellation, and moves a `Draft` Workspace to `Ready`.
- [x] **Native Command Console**: development UI for provider setup, workspace setup, placement selection, provisioning progress sync, cancellation, and recovery/error inspection.
- [ ] **Onboarding UI**: replace the command console with the user-facing setup path for provider setup, workspace setup, placement selection, provisioning progress, cancellation, and recovery states.
- [ ] **Text-to-Image Generator**: build the first generation surface on top of a `Ready` Workspace and the RunPod Endpoint Worker contract. v1 targets the bundled text-to-image workflow rather than arbitrary user-authored ComfyUI workflows.

## App Boundaries

LumaForge keeps UI, local orchestration, and remote worker responsibilities separate:

- The **React frontend** presents setup, provisioning, and generation screens, collects user input, keeps temporary UI state.
- The **Tauri native layer** validates requests, orchestrates provider setup, workspace provisioning, and workflow execution.
- **Workers** run inside provider-managed compute and perform environment preparation or runtime workflow execution behind provider resources.

### Repository Structure

```text
src/
  app/                   React app providers and application composition
  pages/                 Page-level UI
  routes/                TanStack Router route definitions
  shared/                Shared UI primitives and generic utilities
  generated/             Generated frontend contracts; do not edit manually

src-tauri/
  src/commands/          Tauri command adapters and generated binding export
  src/domain/            Domain rules and validation
  src/provider/          GPU provider registry and provider-facing logic
  src/provider_setup/    Provider API key setup workflow
  src/secrets/           Secure secret storage abstraction
  src/workspace_setup/   Workspace setup workflow
  src/workspace_catalog/ Workspace metadata persistence
  src/workspace_resources/
                         Provider resource lifecycle and cleanup operations
  src/workspace_provisioner/
                         Provisioner Worker gateway and environment sync logic
  src/workspace_provisioning/
                         Native workspace provisioning orchestration

spec/
  domain.md              Product/domain overview
  flows/                 Critical flow specifications
  reference/             Type-level reference contracts
  ubiquitous-language/   Domain vocabulary

workers/
  Dockerfile             Shared worker image entrypoint
  provisioner/           Container-side worker for preparing ComfyUI workspaces
  runpod-endpoint/       RunPod endpoint worker for runtime generation
  runtime-recipes/       Runtime image recipe schema and release tooling
```

## Key Flows

- [GPU Cloud Provider Setup](./spec/flows/gpu-cloud-provider-setup.md): validates a provider API key, stores it in the secure keyring, and derives setup status from the stored key and provider identity.
- [Workspace Setup](./spec/flows/workspace-setup.md): creates one local `Draft` Workspace Catalog entry from a Workflow Preset and Placement Plan. It does not create provider resources.
- [Workspace Provisioning](./spec/flows/workspace-provisioning.md): provisions one saved `Draft` Workspace into `Ready` by creating provider resources, preparing the environment, syncing progress, and preserving cleanup metadata on failure.

## Development

| Command                                                                                                | Purpose                                     |
| ------------------------------------------------------------------------------------------------------ | ------------------------------------------- |
| `bun install`                                                                                          | Install frontend dependencies.              |
| `bun run dev`                                                                                          | Start the Vite frontend development server. |
| `bun run tauri dev`                                                                                    | Run the Tauri desktop application.          |
| `bun run build`                                                                                        | Build and type-check the frontend.          |
| `bun run lint`                                                                                         | Run ESLint.                                 |
| `bun run lint:fix`                                                                                     | Apply ESLint autofixes.                     |
| `cargo test --manifest-path src-tauri/Cargo.toml`                                                      | Run native tests.                           |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --fix --allow-dirty --allow-staged`                 | Run native linting with autofixes.          |
| `cargo fmt --manifest-path src-tauri/Cargo.toml`                                                       | Format native code.                         |
| `PYTHONPATH=workers/provisioner/src python3 -m unittest discover -s workers/provisioner/tests`         | Run provisioner worker tests.               |
| `PYTHONPATH=workers/runpod-endpoint/src python3 -m unittest discover -s workers/runpod-endpoint/tests` | Run RunPod endpoint worker tests.           |

## Code Generation

Generated files live in `src/generated` and should not be edited manually.

| Command                        | Purpose                                                                        |
| ------------------------------ | ------------------------------------------------------------------------------ |
| `bun run codegen`              | Regenerate all generated frontend contracts.                                   |
| `bun run codegen:routes`       | Regenerate `src/generated/routeTree.gen.ts` with the TanStack Router CLI.      |
| `bun run codegen:routes:watch` | Watch `src/routes` and regenerate the route tree on changes.                   |
| `bun run codegen:commands`     | Regenerate `src/generated/commands.ts` from Tauri commands via `tauri-specta`. |
