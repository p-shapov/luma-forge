## 1. Native Runtime Contract Stabilization

- [x] 1.1 Add RunPod pod response parsing support for the provider `image` field while preserving non-empty image validation.
- [x] 1.2 Update RunPod pod mapper and provider tests to cover `image`, legacy `imageName`, and missing image identity responses.
- [x] 1.3 Pass `LUMA_FORGE_PROVISIONER_IMAGE_REF` into provisioning pod environment using the Workspace resolved runtime implementation snapshot.
- [x] 1.4 Add workspace provisioning tests proving pod creation uses the selected provisioner image ref for both pod image and non-secret worker environment identity.
- [x] 1.5 Keep Native provisioning pod recovery keyed by provider ownership identity rather than provider-reported image identity.

## 2. Provisioner Runtime Materialization

- [x] 2.1 Export runtime contract id, contract version, implementation revision, and runtime archive path in the final provisioner Docker stage.
- [x] 2.2 Ensure the running Provisioner Worker requires `LUMA_FORGE_PROVISIONER_IMAGE_REF` from pod configuration and rejects start requests that do not match it.
- [x] 2.3 Make Docker archive output and Provisioner Worker extraction use the same Python 3.12-compatible archive format or explicit decompression path.
- [x] 2.4 Publish `.luma-forge/base-runtime` dependency records from runtime staging into final workspace metadata locations.
- [x] 2.5 Write prepared runtime manifest dependency record paths as workspace-resolved paths that point to published files.
- [x] 2.6 Add provisioner tests for runtime identity mismatch, real archive extraction format, base record publishing, missing declared records, and manifest path shape.

## 3. Endpoint Runtime Validation

- [x] 3.1 Update endpoint prepared runtime validation to require dependency record paths to resolve under the workspace mount.
- [x] 3.2 Require every manifest dependency record path to exist before accepting the prepared runtime environment.
- [x] 3.3 Add endpoint tests for valid workspace-resolved record paths, path escape rejection, missing record rejection, and no repair attempts.

## 4. Verification

- [x] 4.1 Run Provisioner Worker unit tests with `PYTHONPATH=src python3 -m unittest discover -s tests` from `workers/provisioner`.
- [x] 4.2 Run Endpoint Worker unit tests with `PYTHONPATH=src python3 -m unittest discover -s tests` from `workers/runpod-endpoint`.
- [x] 4.3 Run `cargo test` from `src-tauri`.
- [x] 4.4 Run `cargo clippy --fix --allow-dirty --allow-staged` from `src-tauri`.
- [x] 4.5 Run `cargo fmt` from `src-tauri`.
- [x] 4.6 Run targeted Docker or release-workflow validation proving the runtime archive format and image metadata paths match the implementation.
