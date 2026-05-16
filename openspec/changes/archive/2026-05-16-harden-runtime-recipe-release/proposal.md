## Why

Runtime recipe releases currently validate that provisioner and endpoint images agree on contract identity, but they do not fully prove that the baked runtime matches the selected recipe or that an existing runtime contract version still has the same compatibility meaning. This can publish digest-pinned worker images whose ComfyUI or PyTorch/CUDA dependency surface no longer matches the bundled Runtime Catalog contract.

## What Changes

- Wire recipe-declared PyTorch package settings into the provisioner Docker build instead of relying on Dockerfile defaults.
- Fail runtime recipe release before publishing if an existing contract id/version would receive an implementation revision built from changed runtime compatibility metadata.
- Generate catalog updates from recipe-derived, verified runtime metadata so the Runtime Catalog contract remains aligned with the baked image.
- Add regression coverage for recipe PyTorch build arguments and existing-contract compatibility checks.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `worker-deployment`: runtime recipe releases must use recipe-declared PyTorch settings and validate existing contract compatibility before publishing.
- `runtime-catalog`: existing runtime contract id/version entries must only receive implementation revisions when the recipe runtime compatibility surface matches the catalog contract.

## Impact

- Affected workflow: `.github/workflows/deploy-runtime-recipe.yml`
- Affected build: `workers/Dockerfile`
- Affected recipe/catalog data: `workers/runtime-recipes/*.yaml`, `bundled/runtime-catalog.json`
- Affected verification: worker deployment tests or scriptable release tooling tests added to cover recipe-to-build and recipe-to-catalog behavior.
