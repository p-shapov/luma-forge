# LumaForge

LumaForge is a desktop application for preparing remote GPU infrastructure and running ComfyUI workflows on it.

The main goal of the product is to turn a local workflow choice into a ready-to-use remote workspace and then use that workspace to execute the selected ComfyUI workflow on remote GPU infrastructure.

## Development

Native backend architecture and extension notes live in [src-tauri/README.md](./src-tauri/README.md).

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
