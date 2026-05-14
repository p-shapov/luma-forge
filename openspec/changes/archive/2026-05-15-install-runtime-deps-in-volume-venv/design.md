## Context

LumaForge provisions ComfyUI on a mounted RunPod network volume and later runs generation through a RunPod Endpoint Worker. The Provisioner Worker is temporary setup compute; its container is deleted after provisioning. The current provisioner installs ComfyUI and Custom Node requirements by invoking the container's default Python, which places dependencies in the provisioner container filesystem rather than the mounted volume.

That means provisioning can report success while the persistent endpoint worker starts from a clean container image with no access to the Python packages installed during provisioning. The endpoint can still see `/workspace/ComfyUI`, models, custom nodes, and workflows, but not the Python environment that was created inside the deleted provisioning pod.

## Goals / Non-Goals

**Goals:**

- Install ComfyUI and Custom Node Python dependencies into a virtual environment stored on the mounted network volume.
- Make the prepared workspace self-describing through runtime metadata, a pip install report, and a frozen dependency record.
- Require endpoint generation to run ComfyUI through the volume-local interpreter.
- Keep worker image base assumptions centralized without making the Provisioner Worker provider-specific.
- Fail clearly when a prepared volume and endpoint runtime are incompatible.

**Non-Goals:**

- No Windows portable Python support in v1.
- No direct use of third-party ComfyUI installer scripts as trusted provisioning logic.
- No guarantee that a Linux virtual environment is portable across operating systems or unrelated container images.
- No React generation UI or user-facing dependency management UI.
- No full dependency lockfile system for every future workflow preset.

## Decisions

### Use a Linux virtual environment on the network volume

The provisioner will create a Linux virtual environment at a stable path such as `/workspace/.venv` and install all ComfyUI and Custom Node Python requirements through `/workspace/.venv/bin/python`. The endpoint worker will use the same interpreter to start ComfyUI.

Alternative considered: install dependencies into the provisioner container. That is the current behavior and fails because the container filesystem is ephemeral.

Alternative considered: use a Windows-style `python_embeded` layout. That pattern is useful conceptually, but the remote runtime is Linux containers on RunPod. A Windows embedded Python distribution would not execute in the RunPod endpoint environment.

### Use a shared provider-neutral worker Docker base

The provisioner and endpoint images will build from a shared provider-neutral worker Dockerfile. The shared base represents the common Linux and Python assumptions needed to create and run the volume-local virtual environment, while provider-specific behavior remains in provider-specific worker targets such as the RunPod endpoint target.

Alternative considered: enforce an explicit runtime profile identifier in both images and the prepared manifest. That would provide a stronger compatibility check, but it prematurely couples the provider-neutral Provisioner Worker to a named runtime profile and makes early iteration more rigid than needed.

### Record resolved environment metadata after installation

After dependency installation, the provisioner will write `/workspace/.luma-forge/runtime.json`, `/workspace/.luma-forge/pip-freeze.txt`, and a pip install report. These files document the interpreter path, Python version, platform, ComfyUI revision, Custom Node revisions, and resolved packages.

Alternative considered: encode exact dependency versions only in the Docker image. That would make endpoint startup predictable, but it would move workflow-specific dependency installation out of the persistent volume and make Custom Node dependency variation harder to support.

### Validate compatibility before generation

The endpoint will validate the runtime manifest, virtual environment interpreter, ComfyUI entrypoint, required workflow files, required model files, and required Custom Node paths before attempting to start ComfyUI. Invalid prepared runtime metadata will be a stable UI-safe failure rather than a late Python import error.

Alternative considered: let ComfyUI startup fail naturally. That produces weak diagnostics and risks leaking internal command output or stack traces.

### Do not run third-party installer packages directly

LumaForge may borrow the portable-environment idea from ComfyUI installer projects, but provisioning logic will remain owned by LumaForge. The provisioner will run explicit Git, venv, and pip operations against validated paths and immutable revisions.

Alternative considered: invoke a third-party "easy install" package during provisioning. That would expand the trusted execution surface and make reproducibility, auditing, and secret-safe diagnostics harder.

## Risks / Trade-offs

- Volume-local venvs are not portable across incompatible images or operating systems -> Build workers from a shared base now and leave stricter compatibility checks to a later change.
- Floating pip resolution can produce different environments over time -> Record install reports and `pip freeze`; add constraints files later when reproducibility needs exceed v1.
- Custom Node dependencies may require system packages or compilers -> Keep common system assumptions in the shared worker base and fail provisioning with a stable dependency error when missing.
- The network volume stores many Python packages and native wheels -> Accept the storage cost to preserve dependencies across the provisioning and endpoint lifecycle.
- Endpoint startup may fail if the venv is partially created or corrupted -> Write metadata only after successful installation and validate required files before generation.

## Migration Plan

This is a pre-production breaking change to the prepared workspace format. Existing prepared volumes without `/workspace/.venv` and `/workspace/.luma-forge/runtime.json` must be reprovisioned before endpoint generation can run.

Implementation should first update the provisioner to create the volume-local environment and manifest, then update endpoint validation/startup to use that interpreter, then update shared worker image build and docs. Rollback is reverting to the previous worker images and reprovisioning any affected workspace volumes.

## Open Questions

- Should v1 use a strict constraints file per shared worker base, or only record the resolved environment after install?
- Should PyTorch be installed into the volume-local venv or provided by the base runtime image and inherited into the venv through explicit configuration?
