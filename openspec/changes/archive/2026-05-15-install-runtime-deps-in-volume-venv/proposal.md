## Why

The current provisioning flow installs ComfyUI and Custom Node Python dependencies into the temporary provisioner container environment, so those dependencies disappear when the provisioning pod is deleted. LumaForge needs the prepared workspace to carry the runtime Python environment on the mounted network volume so the persistent endpoint worker can run the same prepared ComfyUI instance later.

## What Changes

- **BREAKING**: Change provisioning dependency installation so ComfyUI and Custom Node Python dependencies are installed into a volume-local virtual environment, not the provisioner container Python environment.
- Add a prepared runtime environment contract for `/workspace/.venv`, runtime metadata, dependency install reports, and endpoint compatibility validation.
- Require the endpoint worker to start ComfyUI through the volume-local Python interpreter.
- Require provisioning readiness to validate the volume-local runtime environment before reporting success.
- Add a shared provider-neutral worker Docker base that both worker images can build from without making the Provisioner Worker provider-specific.

## Capabilities

### New Capabilities

- `prepared-runtime-environment`: Defines the mounted workspace runtime environment, volume-local virtual environment, runtime manifest, and endpoint compatibility contract.

### Modified Capabilities

- `endpoint-worker`: Endpoint Worker must validate the prepared runtime manifest and launch ComfyUI through the mounted volume virtual environment.
- `provisioner-worker`: Dependency installation and final validation must target the mounted volume virtual environment instead of the ephemeral container environment.
- `worker-deployment`: Worker deployment must build Provisioner Worker and Endpoint Worker images from a shared provider-neutral worker base.

## Impact

- Affected worker code: Provisioner Worker dependency installation, environment validation, metadata writing, and progress/error reporting.
- Affected endpoint code: RunPod Endpoint Worker environment validation and ComfyUI startup command.
- Affected deployment code: worker image build/publish workflow and shared worker Dockerfile.
- Affected specs/docs: provisioner worker, prepared runtime environment, worker deployment, and workspace provisioning flow documentation.
- No React generation UI or arbitrary workflow execution protocol is included in this change.
