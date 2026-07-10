# Rust-Side Layer Refactor Design

## Direction

Use a persistence-first, iterative refactor from current `tauri_api`, `app`,
`workspace`, `domain`, `provider`, catalog, SQLite repository, and secret
storage modules toward layered Rust backend boundaries.

This is an umbrella spec. It defines shared target boundaries and iteration
order only. Each iteration needs its own focused design spec before
implementation planning, including exact contracts, files, tests, and
verification commands.

During intermediate iterations, neighboring layers may be temporarily broken if
the layer under change remains testable through focused tests. Full backend
integration returns in the final iteration. No legacy bridges, compatibility
shims, or migrations are added for the old pre-v1 JSON persistence schema.
Incompatible existing dev DBs fail clearly.

## Iterations

1. **Persistence**
   Add SeaORM under `infra/sqlite/entities/*` and
   `infra/sqlite/repositories/*`. Replace JSON-backed workspace/lifecycle
   repositories with normalized relational repositories for `workspaces`,
   `workspace_runtimes`, `runpod_workspace_runtimes`, `lifecycle_operations`,
   and `runpod_operation_payloads`. Remove `runtime_json` and `payload_json`
   from the target contract.

2. **Bundled Catalogs**
   Add runtime filesystem catalog loading under `infra/bundled` for
   `new_bundled/catalog`. `infra/bundled` reads catalog contracts, schemas, and
   entries from an injected root path, validates entries through declared JSON
   Schemas, resolves declarative references, and exposes persistence-free
   repositories under `infra/bundled/repositories`. Old `bundled/**` stays
   untouched and is not used as a fallback.

3. **Infra Keyring And Providers**
   Move technical secure storage and raw provider HTTP clients into
   `infra/keyring/*`, `infra/providers/runpod/*`, and
   `infra/providers/hugging_face/*`. These modules own platform keyring access,
   raw HTTP calls, provider request/response mapping, and provider identity
   calls. They do not implement application workspace ports in this iteration.

4. **Application Workspace**
   Replace current workspace module with `application/workspace`: `model.rs`, `ports/{runtime.rs, repository.rs, catalog.rs}`, `errors.rs`, `service.rs`, `background.rs`, `mod.rs`. It owns provider-neutral workspace and lifecycle operation creation, state transitions and in-flight tracking.

5. **RunPod Runtime**
   Move RunPod lifecycle orchestration into `runtime/runpod`: `model.rs`,`ports/{secrets.rs, provider.rs,repository.rs, catalog.rs}`, `provision.rs`, `cleanup.rs`, `delete.rs`, `runtime.rs`, `mod.rs, errors.rs`. It owns step order, provisioning/cleanup order, provisioner polling behavior, RunPod payload and runtime persistense, and progress reporting through workspace's port.

6. **Adapters for Workspace and RunPod Runtime**
   We should to define layer(s) for adapters with

   `sectets/{runpod,hugging_face}`: `adapters/{runpod}`

7. **Facade And Composition**
   Move Tauri/Specta API boundary into
   `facade/{commands/*,events.rs,types/*, errors.rs,tracing.rs}` and dependency
   wiring into `composition/{bootstrap.rs,state.rs}`. Command DTOs/events may
   change. `src/generated/commands.ts` is updated by codegen only.

## Dependency Rule

Final dependency direction:

```text
facade -> application
application -> application models + application ports
runtime/runpod -> runtime models + runtime ports
secrets -> application ports + secrets ports
infra -> application/runtime models + application/runtime/secrets ports
composition -> wires all
```

Layer ownership:

- `facade`: Tauri commands/events, Specta DTOs, UI-safe errors, `traceId`.
- `application`: provider-neutral workspace use cases and lifecycle state.
- `runtime/runpod`: RunPod lifecycle sequence and RunPod-specific runtime data.
- `infra`: SeaORM SQLite repositories, bundled readers, keyring, HTTP clients,
  and Tauri event sink implementation.
- `secrets`: credential workflows using narrow secrets-owned storage and
  provider identity ports.
- `composition`: DB open, diagnostics init, concrete dependency wiring, and
  `NativeAppState`.

No final `domain` layer. Provider-neutral models live in
`application/workspace/model.rs`; RunPod-specific models live in
`runtime/runpod/model.rs`.

## Verification

Each iteration gets focused design, implementation plan, and verification
commands.

- Iteration 1: SeaORM bootstrap, workspace/runtime mapping,
  operation/RunPod-payload mapping, repository transactions, no JSON columns.
- Iteration 2: bundled catalog loading and validation boundaries.
- Iteration 3: keyring/provider infrastructure boundaries.
- Iteration 4: application workspace models, ports, and use cases with fakes.
- Iteration 5: `runtime/runpod` sequencing, provider failures, runtime persistence, and typed events with fake runtime ports.
- Iteration 6: secrets adapters against fake keyring/provider infrastructure.
- Iteration 7: facade, composition, codegen, and full backend integration.

Full final verification:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
bun run codegen:commands
bun run build
bun run lint
```

Do not add tests for removed JSON persistence, deprecated module names,
compatibility paths, old command DTOs, or absence of removed behavior.
