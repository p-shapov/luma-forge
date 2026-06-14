# Worker Deployment

LumaForge worker images are released through separate GitHub Actions workflows:

- Provisioner Worker releases publish the generic workspace-preparation image.
- Endpoint contract releases publish RunPod Endpoint Worker images for a selected endpoint contract.

RunPod Endpoint Worker deployment is driven by [`promote-runpod-endpoint/`](./promote-runpod-endpoint/). That module owns runtime preset YAML, schema validation, endpoint image build metadata resolution, and RunPod endpoint promotion. [`promote-provisioner/`](./promote-provisioner/) owns provisioner promotion.

## Release Triggers

- Push a provisioner release tag matching `provisioner-worker-v*`, or run `Deploy Provisioner` manually, to publish the generic provisioner image.
- Push a RunPod endpoint release tag matching `runpod-endpoint-v*`, or run `Deploy RunPod Endpoint` manually with `workflow_id` and `workflow_version`. The workflow resolves that revision's `runtime_preset`, for example `comfyui-py312-cu126-torch291`.

Manual endpoint contract releases publish a workflow-specific endpoint image, then automatically propose Runtime Contracts promotion under the selected endpoint contract id. The workflow resolves the next endpoint runtime contract patch version from `bundled/runtime-contracts.json`, for example `1.0.0` to `1.0.1`, before endpoint worker validation, image builds, or publication. If the contract declares a higher SemVer version than the next patch, the workflow uses the contract version instead.

Manual provisioner releases publish a provisioner image, then automatically propose catalog promotion under `provisioner`. The workflow resolves the next provisioner contract patch version from `bundled/runtime-contracts.json`, for example `1.0.0` to `1.0.1`, before provisioner validation, image build, or publication.

Do not publish another worker image for the same contract while a promotion PR for that contract remains open. Endpoint and provisioner releases choose the next patch version from the current bundled contracts, so concurrent releases for the same contract can compute the same next version.

## Registry

The workflows publish to GitHub Container Registry:

- `ghcr.io/<owner>/<repo>/runpod-endpoint-worker`
- `ghcr.io/<owner>/<repo>/provisioner-worker`

Published image refs are resolved after `docker push` as digest-pinned refs such as `ghcr.io/<owner>/<repo>/runpod-endpoint-worker@sha256:<digest>` and `ghcr.io/<owner>/<repo>/provisioner-worker@sha256:<digest>`. Catalog revisions store those immutable refs; mutable tags are not accepted by the release tooling.

Do not store registry credentials, provider API keys, or worker bearer tokens in repository files. The deployment workflows read authentication only from GitHub Actions token context.

## Validation

The provisioner workflow validates only the provisioner package and builds the generic provisioner image without endpoint contract build arguments.

The endpoint contract workflow validates endpoint contract tooling and the endpoint package, then builds the endpoint image with the selected contract dependencies and bundled workflow file. The endpoint contract id matches the Workflow Preset id; release tooling derives `bundled/workflows/{contract.id}.json`, and the Docker build copies that source workflow to `/opt/luma-forge/runtime/workflows/workflow.json`. It does not require live ComfyUI execution.

## Catalog Promotion

Publishing an image does not make new Workspaces select it. Selection changes only after the corresponding catalog promotion PR is reviewed, merged, and bundled into the app.

After publishing a validated provisioner image, the workflow opens a reviewed Runtime Contracts promotion PR that appends the selected provisioner contract id/version revision in `bundled/runtime-contracts.json` with a digest-pinned provisioner image ref. The same PR updates `bundled/workflow-catalog.json` so Workflow Presets using that provisioner contract id point at the new revision.

After publishing a validated endpoint image, the workflow opens a reviewed Runtime Contracts promotion PR that appends the selected endpoint contract id/version revision in `bundled/runtime-contracts.json` with a digest-pinned endpoint image ref. The same PR updates `bundled/workflow-catalog.json` so the Workflow Preset whose id matches the endpoint contract id points at the new revision.

Catalog promotion PRs are path-guarded. Provisioner and endpoint PRs may change only `bundled/runtime-contracts.json` and `bundled/workflow-catalog.json`.

Workflow Presets remain exact-pinned to endpoint contract id/version pairs. The HiDream O1 Dev preset uses a 120 GiB base volume requirement to cover the Dev checkpoint, Gemma text encoder, outputs, and operational workspace headroom.

## Rollback

Rollback by opening a reviewed change to `bundled/workflow-catalog.json` that points Workflow Presets back to a previously published Runtime Contracts revision, or by publishing a newer worker image and merging the resulting catalog revision. Do not mutate existing catalog revisions, and do not repoint persisted Workspace snapshots. Existing Workspaces remain pinned to their persisted runtime and provisioner image snapshots.
