## 1. Domain Snapshot Contract

- [x] 1.1 Remove `runtime_env` from `RunPodEndpointTemplateSnapshot` in the workspace domain model.
- [x] 1.2 Update provisioning snapshot conversion so provider-returned endpoint template env values are discarded before Workspace metadata is persisted.
- [ ] 1.3 Verify legacy Workspace metadata containing `runtime_env` still deserializes and is rewritten without that field on subsequent persistence.

## 2. Command Boundary and Bindings

- [x] 2.1 Remove `runtime_env` from the command-owned remote `RunPodEndpointTemplateSnapshot` binding metadata.
- [x] 2.2 Regenerate TypeScript command bindings with `bun run codegen:commands`.
- [x] 2.3 Remove or update any frontend references to endpoint template `runtime_env`.

## 3. Provisioning Behavior Tests

- [ ] 3.1 Update Workspace Provisioning tests so created, adopted, and refreshed template observations with secret-like env values persist only safe template metadata.
- [ ] 3.2 Replace the existing reuse test that accepts extra runtime env keys with an assertion that the values are omitted from the resulting Workspace snapshot.
- [ ] 3.3 Add or update command contract/binding tests to prove Workspace responses do not expose `runtime_env`.

## 4. Verification

- [x] 4.1 Run `cargo test`.
- [x] 4.2 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 4.3 Run `cargo fmt`.
- [x] 4.4 Run `bun run build`.
- [x] 4.5 Run `bun run lint --fix`.
