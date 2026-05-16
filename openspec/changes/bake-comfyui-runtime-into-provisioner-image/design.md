## Context

LumaForge currently prepares ComfyUI workspaces by cloning the ComfyUI revision declared by the selected Workflow Preset, creating `/workspace/.venv`, installing ComfyUI requirements, installing Custom Node requirements, and writing runtime metadata during provisioning. The RunPod Endpoint Worker then validates that metadata and starts ComfyUI through `/workspace/.venv/bin/python`.

That design fixed endpoint/provisioner filesystem separation, but it still makes every provisioning run rebuild the stable Python/PyTorch/ComfyUI base runtime. It also uses Workflow Presets as ComfyUI runtime installation instructions. The new direction is to make Workflow Presets require named runtime contracts, make bundled runtime catalog entries resolve those contracts to verified immutable worker image-pair implementation revisions, and move deterministic base runtime construction into Docker image builds while preserving the endpoint's simple mounted-workspace execution model.

## Goals / Non-Goals

**Goals:**

- Introduce a bundled runtime catalog that maps runtime contract id/version pairs to verified runtime metadata and immutable provisioner/endpoint image-pair implementation revisions.
- Change Workflow Presets to require runtime contracts instead of specifying ComfyUI Git revisions as install instructions.
- Store a resolved runtime contract implementation snapshot on Workspaces so they stay pinned as newer runtime contracts or implementation revisions are added.
- Build one fixed Python + PyTorch + CUDA-compatible + ComfyUI base runtime archive into the Provisioner Worker image for each runtime recipe release.
- Include ComfyUI source, frontend/docs/templates, and base ComfyUI requirements in the build-time runtime archive.
- Keep Workflow Preset Custom Nodes as a provisioning-time layer because their set varies by workflow and installing them is not expected to dominate provisioning time.
- Build the virtual environment with final prefix `/workspace/.venv`, then package the base runtime into a deterministic archive in the image so provisioning can extract it onto the mounted volume without relocating paths.
- Keep provisioning responsible for extracting the baked runtime archive, installing or verifying Workflow Preset Custom Nodes, writing workspace runtime metadata, and downloading or verifying model assets.
- Keep endpoint generation running from the mounted workspace runtime after provisioning.
- Keep GPU selection separate from base runtime and Custom Node Python dependency installation.
- Deploy runtime recipe implementations by building, validating, publishing, and cataloging compatible provisioner/endpoint image-pair implementation revisions.

**Non-Goals:**

- Do not make the Endpoint Worker image carry the full ComfyUI runtime in this change.
- Do not introduce dynamic per-GPU Python dependency resolution.
- Do not bake Workflow Preset Custom Nodes into runtime contract Docker images.
- Do not add a general dependency lockfile system for arbitrary future user-authored Custom Nodes.
- Do not change RunPod from the only v1 provider.
- Do not support already-prepared pre-production volumes created with the old provisioning-time dependency installation format.
- Do not introduce version ranges for Workflow Preset runtime requirements in the first implementation.

## Decisions

### Add a bundled runtime catalog

Add `bundled/runtime-catalog.json` beside the existing workflow catalog. Runtime catalog entries should use stable contract identifiers and exact versions, for example `comfyui-python312-cu121@1.0.0`, and should include runtime metadata, the ComfyUI/base runtime revisions included in the image-baked runtime, a list of immutable provisioner/endpoint image-pair implementation revisions, and a default implementation revision for new Workspaces.

Rationale: Workflow authors and future custom workflow UX need to reason about compatibility at the level of a named ComfyUI Python/CUDA runtime rather than raw Docker images, arbitrary ComfyUI commits, or one-off workflow-specific recipes.

Runtime contract terminology should stay split:

- Runtime recipe: tracked build input that declares the intended contract id/version and base runtime ingredients.
- Runtime Catalog entry: reviewed application data for one contract id/version, including verified runtime metadata plus one or more immutable implementation revisions.
- Runtime implementation revision: release-assigned identifier for one validated provisioner/endpoint image pair under a contract id/version. Worker-only fixes append a new implementation revision without changing runtime compatibility.
- Resolved runtime contract implementation snapshot: Workspace-persisted copy of a Runtime Catalog entry's selected implementation revision.

Alternative considered: put Docker image refs directly in Workflow Presets. That is simple but couples workflow intent to deployment artifacts and makes custom workflow compatibility difficult to explain.

Alternative considered: create one image per ComfyUI revision and let presets reference revisions. That is reproducible but too granular as the primary abstraction; most compatibility changes involve a runtime surface, not only one ComfyUI commit.

### Resolve runtime contracts during Workspace Setup

Workflow Presets should declare an exact `required_runtime_contract` id/version. Workspace Setup should validate that the referenced runtime contract exists in the bundled runtime catalog, resolve its current default implementation revision, then persist a resolved runtime implementation snapshot on the Draft Workspace alongside the selected Workflow Preset snapshot.

Rationale: already-created Workspaces remain pinned to the runtime contract, implementation revision, and immutable image refs they were created with, even if later app builds add newer contracts or newer implementation revisions.

Alternative considered: resolve the runtime contract during provisioning only. That makes Draft Workspaces less durable and can silently change their runtime if catalogs change between setup and provisioning.

### Move worker image refs out of native build configuration

Native build configuration should stop requiring global Provisioner Worker and Endpoint Worker image refs. Image refs are release outputs of runtime implementation revisions and should be read from the Workspace's persisted runtime implementation snapshot. Native build configuration may keep non-image worker values such as worker ports until those values are also moved into verified image metadata.

Rationale: global build-time image refs conflict with per-runtime implementation snapshots. Keeping image refs in one source of truth prevents provisioning from accidentally creating resources with images that do not match the selected runtime contract.

Alternative considered: leave build-time image refs as defaults and let runtime snapshots override them. That preserves the old shape, but it creates two competing image sources and makes fallback behavior risky.

### Treat runtime contract versions and implementation revisions as immutable

Once `contract_id@version` is published, its compatibility meaning should not be mutated in place. New compatible or breaking runtime changes should add a new version. Worker-only changes that do not alter the compatibility surface should append a new immutable implementation revision under the same contract id/version and may advance the catalog's default implementation revision for future Workspaces.

Rationale: old Workflow Presets remain usable by keeping old runtime contracts available, and old Workspaces remain reproducible by keeping their selected implementation revision and image refs pinned.

Alternative considered: update a contract version in place to point at newer images. That breaks reproducibility and makes failures hard to diagnose because persisted Workspaces could silently move to a different worker implementation.

### Materialize a build-time runtime archive into the workspace

The Provisioner Worker image for the resolved runtime contract implementation will contain a compressed tar archive for the base runtime with ComfyUI and a packaged virtual environment. Provisioning will extract that archive into a staging path on the mounted workspace volume, verify required files and metadata, then publish the staged runtime to `/workspace/ComfyUI` and `/workspace/.venv`. The runtime manifest is written only after materialization, Custom Node preparation, asset download, and final validation succeed.

Rationale: a single archive is easier to checksum, preserves executable modes, handles many small Python files more predictably, and avoids partial directory-copy state. Staging plus final publish keeps failed extraction from looking like a valid prepared runtime.

Alternative considered: bake the full runtime into both provisioner and endpoint images and run ComfyUI directly from the endpoint image. That gives a stronger image-local runtime contract, but it requires changing model/workflow path semantics and endpoint image shape at the same time.

Alternative considered: keep installing the full runtime into the workspace during provisioning. That is the current behavior and conflicts with the goal of removing heavy base runtime installation from provisioning.

Alternative considered: copy or sync image directories directly into the mounted workspace. That is simple, but it is harder to validate as one deterministic artifact and easier to leave partially copied runtime trees after interruption.

### Install Workflow Preset Custom Nodes during provisioning

Workflow Preset Custom Nodes should remain provisioning-time inputs. The selected preset can declare the Custom Node sources, revisions, safe install paths, and dependency install instructions needed by that workflow. After the base runtime archive is materialized, provisioning may clone or copy those preset-declared Custom Nodes into `/workspace/ComfyUI/custom_nodes`, install their requirements into the materialized `/workspace/.venv`, verify the result, and record the installed nodes in the runtime manifest.

Rationale: Custom Node sets vary by workflow, and baking every preset-specific node into the runtime image would either overstuff images or require too many image variants. Installing only the selected workflow's nodes is a smaller variable layer than rebuilding the base Python/PyTorch/ComfyUI runtime for every workspace.

Alternative considered: include Custom Nodes in the runtime contract image. That keeps provisioning more static, but couples runtime image versions to workflow-specific node sets and makes future custom workflow support harder to explain.

### Build the virtual environment at its final path

The Docker build should create the Python environment at `/workspace/.venv` or an equivalent staged path whose shebangs and `pyvenv.cfg` point to `/workspace/.venv`, then package that directory into the image. Provisioning will extract it to the same path on the mounted volume.

Rationale: Python virtual environments are not reliably relocatable. Building with the final prefix avoids trying to force relative venv paths.

Alternative considered: create `/opt/luma-forge/.venv` and copy it to `/workspace/.venv` during provisioning. That risks absolute-path breakage in console scripts, pip metadata, and `pyvenv.cfg`.

### Treat provisioning as materialization plus verification

The provisioner will not run `python -m venv`, `pip install` for the base ComfyUI runtime, or Git checkout commands for ComfyUI itself. It may remove or overwrite incomplete materialized runtime directories, extract the baked archive, install or verify preset-declared Custom Nodes, verify required files, capture UI-safe metadata, and download model assets.

Rationale: extracting a deterministic base archive keeps provisioning from rebuilding the heavy stable runtime while still allowing the workflow-specific Custom Node layer to vary by preset.

Alternative considered: leave the existing git checkout path and only skip pip. That still makes ComfyUI source selection a runtime operation and weakens the image-declared contract.

### Adopt only runtime-compatible provisioning pods

When recovering from missing local pod state or indeterminate pod creation, Workspace Provisioning should treat a discovered RunPod provisioning pod as safe to adopt only if provider-visible metadata proves the stable Workspace-derived pod name, network volume id, and immutable Provisioner Worker image ref from the Workspace's runtime implementation snapshot. A live correlated pod with the wrong image, or one whose image cannot be proven, should fail closed with UI-safe provider metadata rather than being adopted or replaced blindly.

Rationale: once provisioner images become runtime-specific, pod name plus volume id are no longer sufficient correlation keys. Adopting a stale pod running a different runtime implementation can prepare the wrong environment or fail after Native has already persisted the wrong active pod.

Alternative considered: rely only on Provisioner Worker start-request validation to catch mismatches. That is a useful second line of defense, but it happens after adoption and makes recovery behavior less precise.

### Runtime metadata records base runtime and Custom Node layers

The workspace runtime manifest should describe that the base environment was materialized from the resolved runtime contract implementation's image-baked runtime archive. It should include the environment kind, runtime contract id/version, implementation revision, concrete image identity, Python path, ComfyUI root, Python version, ComfyUI revision, preset-installed Custom Node revisions, build-time dependency record paths or digest references, and materialization timestamp.

Rationale: the endpoint needs to validate that the mounted runtime has the expected shape while distinguishing the image-baked base runtime from the preset-specific Custom Node layer installed during provisioning.

Alternative considered: preserve `pip-freeze.txt` and pip install report semantics exactly. That would keep old tooling familiar, but it blurs which dependencies came from the image-baked base runtime versus the preset-specific Custom Node layer.

### Keep endpoint lightweight

The Endpoint Worker remains responsible for validating the prepared workspace and starting `/workspace/.venv/bin/python /workspace/ComfyUI/main.py`. It still must not repair the workspace by installing dependencies, cloning repositories, or downloading assets.

Rationale: this preserves the current runtime boundary and lets the first implementation focus on the provisioner image and preparation path.

Alternative considered: make the endpoint image the only runtime owner. That is a plausible future simplification but makes this change larger than necessary.

### Release runtime recipe implementations as image pairs

Replace independent publish workflows for provisioner and endpoint runtime images with a runtime recipe release workflow. The workflow should build both images from a selected runtime recipe, validate endpoint/provisioner contract metadata compatibility, publish immutable image refs, and open a reviewed PR updating `bundled/runtime-catalog.json`. If the recipe declares a new contract id/version, the PR should add a new Runtime Catalog entry. If the recipe redeploys an existing contract id/version without changing runtime compatibility, the PR should append a new immutable implementation revision and advance the default implementation revision for future Workspaces.

Rationale: a runtime contract is implemented by a matched provisioner/endpoint pair, but the catalog entry or implementation revision should only be created after that pair has been built and validated. Publishing images independently can create manifest-shape and runtime-contract mismatches.

Alternative considered: keep separate deployment workflows and manually edit the runtime catalog. That keeps workflows small but makes catalog/image drift likely.

### Runtime recipes are YAML CI inputs

Runtime recipes should be flat YAML files under a worker-owned directory, such as `workers/runtime-recipes/comfyui-python312-cu121.yaml`, validated by a JSON Schema in CI. A recipe declares build intent: runtime contract id/version, Python version, PyTorch/CUDA package line, ComfyUI source revision, base requirements, and build metadata. It must not contain published image digests because those are produced after validation and recorded in the Runtime Catalog.

Rationale: YAML is readable for release review, while a schema keeps the CI input strict. Keeping recipes out of app runtime data avoids treating unvalidated build intent as the source of truth for Workspace creation.

Alternative considered: make `bundled/runtime-catalog.json` the build input. That collapses intent and verified output, and it cannot contain final immutable image refs before the images are built.

### GPU placement does not select dependencies

For v1, selected GPU validation should remain provider placement validation and must not select, add, remove, or reinstall Python packages based on GPU choice. The Runtime Catalog should not require detailed GPU capability metadata until LumaForge has a reliable provider metadata source and concrete workflow requirements for those checks.

Rationale: base runtime dependencies are a property of the image contract, and preset Custom Node dependencies are a property of the selected workflow. Neither should be selected by the user's RunPod GPU choice.

Alternative considered: add runtime contract GPU requirements immediately. That creates false precision while RunPod metadata reliability and actual workflow capacity requirements are still unclear.

## Risks / Trade-offs

- Prebuilt venv contains hidden absolute-path assumptions -> Build it with final `/workspace/.venv` prefix and cover materialization behavior with focused worker tests; keep full archive extraction smoke checks manual because the release build already installs the complete ComfyUI dependency set.
- Materializing ComfyUI and the venv duplicates runtime bytes per workspace -> Accept the storage cost for v1 to keep endpoint execution simple and persistent across serverless cold starts.
- Provisioning-time Custom Node installs can introduce package conflicts -> Require pinned preset-declared node sources and fail validation instead of mutating the runtime contract or selecting packages from GPU placement.
- Partial extraction can leave corrupt workspaces -> Extract archive contents into temporary paths and only publish metadata after validation succeeds, or clean known runtime directories before materialization.
- Provisioner image becomes significantly larger -> Use deterministic image tags and deployment validation so rollback is selecting an earlier image ref.
- Runtime images and bundled runtime catalog can drift -> Generate or update runtime catalog entries and implementation revisions from validated image metadata through a reviewed PR instead of hand-editing image refs after publish.
- Workspace Presets can reference missing runtime contracts -> Treat the bundled catalog set as invalid and fail Workflow Catalog reads or Workspace creation before persistence.
- Old runtime implementations can become unavailable in the registry -> Use immutable image refs/digests and retain published image versions as long as any bundled catalog entry or persisted Workspace can reference them.
- RunPod GPU metadata can be incomplete -> Keep v1 placement checks conservative and avoid dependency selection based on GPU metadata.

## Migration Plan

This is a pre-production breaking change to the prepared workspace format. Existing prepared volumes that rely on provisioning-created `/workspace/.venv` metadata should be reprovisioned.

Implementation should add runtime catalog/domain support first, then update Workflow Preset contracts and Workspace persistence, then update Docker build/runtime archive creation, provisioner materialization, endpoint manifest validation, unified deployment, docs, and tests. Rollback is selecting a previous implementation revision in the Runtime Catalog for future Workspaces or reprovisioning any workspaces created with an incompatible format.

## Open Questions

- None for the current proposal.
