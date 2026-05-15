## 1. RunPod Inventory Mapping

- [x] 1.1 Extend the RunPod inventory GraphQL query to request datacenter `storageSupport`.
- [x] 1.2 Add a nullable storage-support field to the RunPod inventory datacenter response contract.
- [x] 1.3 Filter RunPod inventory mapping so only datacenters with `storageSupport == true` are returned.
- [x] 1.4 Preserve the existing provider-neutral `ProviderInventory` and generated command response shape unless implementation proves an explicit capability field is required.

## 2. Tests

- [x] 2.1 Add RunPod mapper coverage for omitting a GPU-available datacenter with `storageSupport: false`.
- [x] 2.2 Add RunPod mapper coverage for omitting a GPU-available datacenter with missing or null `storageSupport`.
- [x] 2.3 Add RunPod mapper coverage proving a storage-supported datacenter with GPU availability is still returned.
- [x] 2.4 Update existing inventory parser fixtures to include storage support where needed.

## 3. Verification

- [x] 3.1 Run `cargo test` for native changes.
- [x] 3.2 Run `cargo clippy --fix --allow-dirty --allow-staged` for native changes.
- [x] 3.3 Run `cargo fmt` for native changes.

## 4. Frontend Availability Display

- [x] 4.1 Display GPU availability labels in Workspace Setup GPU options.
- [x] 4.2 Display the selected GPU availability score in the GPU field description.
- [x] 4.3 Run `bun run build` for frontend changes.
- [x] 4.4 Run `bun run lint --fix` for frontend changes.

## 5. Frontend Availability Guard

- [x] 5.1 Disable workspace creation when the selected GPU has zero availability.
- [x] 5.2 Disable provisioning start when loaded placement options show the selected workspace GPU is unavailable.
- [x] 5.3 Keep provisioning sync and cancellation available for existing workspaces.
- [x] 5.4 Run `bun run build` for frontend changes.
- [x] 5.5 Run `bun run lint --fix` for frontend changes.
