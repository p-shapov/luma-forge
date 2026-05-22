# Worker Deployment

LumaForge worker images are released through separate GitHub Actions workflows:

- Provisioner Worker releases publish the generic workspace-preparation image.
- Runtime contract releases publish RunPod Endpoint Worker images for a selected endpoint runtime contract.

RunPod Endpoint Worker deployment is driven by runtime contracts under [`runtime-contracts/`](./runtime-contracts/).

## Release Triggers

- Push a provisioner release tag matching `provisioner-worker-v*`, or run `Deploy Provisioner Worker` manually, to publish the generic provisioner image.
- Push a runtime contract release tag matching `runtime-contract-v*`, or run `Deploy Runtime Contract` manually and select one contract, for example `workers/runtime-contracts/comfyui-python312-cu121.yaml`.

Manual runtime contract releases append a new Runtime Catalog revision under the selected runtime contract id. The workflow resolves the next patch version from `bundled/runtime-catalog.json`, for example `1.0.0` to `1.0.1`, before endpoint worker validation, image builds, or publication. If the contract declares a higher SemVer version than the next patch, the workflow uses the contract version instead.

Manual provisioner releases append a new Provisioner Catalog revision under `luma-forge-provisioner`. The workflow resolves the next patch version from `bundled/provisioner-catalog.json`, for example `1.0.0` to `1.0.1`, before provisioner validation, image build, or publication.

Do not publish another worker image for the same catalog contract while a catalog bump PR for that contract remains open. Runtime and provisioner releases choose the next patch version from the current bundled catalog, so concurrent releases for the same contract can compute the same next version.

## Registry

The workflows publish to GitHub Container Registry:

- `ghcr.io/<owner>/<repo>/runpod-endpoint-worker`
- `ghcr.io/<owner>/<repo>/provisioner-worker`

Published image refs are resolved after `docker push` as digest-pinned refs such as `ghcr.io/<owner>/<repo>/runpod-endpoint-worker@sha256:<digest>` and `ghcr.io/<owner>/<repo>/provisioner-worker@sha256:<digest>`. Catalog revisions store those immutable refs; mutable tags are not accepted by the release tooling.

Do not store registry credentials, provider API keys, or worker bearer tokens in repository files. The deployment workflows read authentication only from GitHub Actions token context.

## Validation

The provisioner workflow validates only the provisioner package and builds the generic provisioner image without runtime contract build arguments.

The runtime contract workflow validates runtime contract tooling and the endpoint package, then builds the endpoint image with the selected contract dependencies. It does not require live ComfyUI execution.

## Catalog Updates

After publishing a validated provisioner image, the workflow opens a reviewed PR that appends the selected provisioner contract id/version revision in `bundled/provisioner-catalog.json` with a digest-pinned provisioner image ref. The same PR updates `bundled/workflow-catalog.json` so Workflow Presets using that provisioner contract id point at the new revision. The Provisioner Catalog bump preserves required revision metadata such as `volume_mount_path`.

After publishing a validated endpoint image, the workflow opens a reviewed PR that appends the selected runtime contract id/version revision in `bundled/runtime-catalog.json` with a digest-pinned endpoint image ref. The same PR updates `bundled/workflow-catalog.json` so Workflow Presets using that runtime contract id point at the new revision.

Catalog update PRs are path-guarded. Provisioner PRs may change only `bundled/provisioner-catalog.json` and `bundled/workflow-catalog.json`; runtime PRs may change only `bundled/runtime-catalog.json` and `bundled/workflow-catalog.json`.

## Rollback

Rollback by opening a reviewed change to `bundled/workflow-catalog.json` that points Workflow Presets back to a previously published Runtime Catalog or Provisioner Catalog revision, or by publishing a newer worker image and merging the resulting catalog revision. Do not mutate existing catalog revisions, and do not repoint persisted Workspace snapshots. Existing Workspaces remain pinned to their persisted runtime and provisioner image snapshots.
