## Why

The Rust native layer currently lets domain models, command DTOs, bundled catalog DTOs, and provider-specific RunPod configuration bleed across module boundaries. This makes the upcoming workspace provisioning work riskier because provider details and generated frontend contracts can spread into core domain code instead of staying behind explicit adapters.

## What Changes

- Move RunPod-specific profile/config contracts under the provider boundary, not into `domain`.
- Remove generated frontend binding and serialization derives from pure domain models.
- Introduce explicit command DTOs and mapper functions between command responses, application snapshots, provider contracts, and domain models.
- Keep bundled catalog reading as infrastructure that parses catalog data into provider/domain-safe types rather than owning workspace contract types.
- Update workspace setup inventory error classification so invalid or revoked provider keys are reported as provider setup recovery errors instead of retryable provider API outages.
- Preserve current workspace creation semantics: duplicate Workspace UUID remains an error, and live data center/GPU availability is still validated later by provider-owned flows.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `native-boundaries`: Clarify pure domain ownership, command DTO mapping, provider-owned RunPod profile/config contracts, and serialization/binding boundaries.
- `workspace-setup`: Classify invalid or revoked provider keys during provider inventory lookup as setup recovery failures, not transient provider API failures.

## Impact

- Affected native modules: `src-tauri/src/domain`, `src-tauri/src/workspace`, `src-tauri/src/bundled`, `src-tauri/src/provider`, and `src-tauri/src/commands`.
- Generated TypeScript bindings may change type ownership and paths, but command-level behavior should remain compatible except for the corrected invalid-key inventory error code.
- No new runtime dependencies are expected.
- Verification: `cargo test`, `cargo clippy --fix --allow-dirty --allow-staged`, `cargo fmt`, and frontend build/lint if generated bindings change under `src/`.
