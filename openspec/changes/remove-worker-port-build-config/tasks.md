## 1. Remove Native Build Port Configuration

- [x] 1.1 Remove obsolete worker env entries from root `.env` and `.env.example`.
- [x] 1.2 Remove the Tauri build-time app config parser under `src-tauri/build/app_config`.
- [x] 1.3 Simplify `src-tauri/build.rs` so it no longer parses or emits worker port Cargo environment values.
- [x] 1.4 Remove `src-tauri/src/app_config` and all `NativeAppConfig` construction from native app state.

## 2. Move Ports To Provisioning/Provider Boundary

- [x] 2.1 Remove worker port fields from `WorkspaceProvisioningConfig` and update service construction accordingly.
- [x] 2.2 Add fixed Provisioner Worker HTTP port ownership near the RunPod provisioning pod creation path.
- [x] 2.3 Replace generic endpoint worker port naming with internal RunPod endpoint ComfyUI HTTP port naming where template port configuration remains necessary.
- [x] 2.4 Verify whether RunPod serverless template creation still needs a `ports` entry; keep, rename, or remove the provider request field based on current behavior.

## 3. Update Tests And Contracts

- [x] 3.1 Update workspace provisioning service tests that currently inject or assert configurable worker ports.
- [x] 3.2 Update RunPod provider registry and client tests for provisioning pod and serverless template request construction.
- [x] 3.3 Update any native build or config tests to assert worker port env vars are not required.
- [x] 3.4 Search for remaining `LUMA_FORGE_PROVISIONER_WORKER_PORT`, `LUMA_FORGE_RUNPOD_ENDPOINT_WORKER_PORT`, and misleading `endpoint_worker_port` references and resolve them.

## 4. Verify

- [x] 4.1 Run `cargo test`.
- [x] 4.2 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 4.3 Run `cargo fmt`.
- [x] 4.4 Re-run targeted searches to confirm worker port values are no longer native build configuration.
