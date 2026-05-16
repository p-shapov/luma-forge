## Why

The runtime recipe release workflow currently pre-fills manual dispatch with an implementation revision that already exists in the bundled Runtime Catalog. A default manual run therefore fails catalog validation before it can build, publish, or open a catalog update PR.

## What Changes

- Remove the known-duplicate manual dispatch default for runtime implementation revisions.
- Ensure manual runtime recipe dispatch either requires an operator-provided revision that is not already present in the selected contract, or derives a fresh revision before validation.
- Keep existing catalog duplicate revision validation as the final guard before publishing.
- Update deployment documentation so operators know how implementation revisions are selected for manual releases.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `worker-deployment`: Manual runtime recipe dispatch must not default to a known duplicate implementation revision and must fail before image build or publication when the selected revision already exists.

## Impact

- Affected workflow: `.github/workflows/deploy-runtime-recipe.yml`
- Affected tooling: `workers/runtime-recipes/release_tool.py`, only if fresh revision derivation is implemented in the release helper
- Affected documentation: `workers/provisioner/DEPLOYMENT.md`
- No frontend, native runtime, provider API, or catalog schema changes are expected.
