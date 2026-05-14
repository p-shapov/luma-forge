## 1. Runtime Contract

- [x] 1.1 Add a prepared runtime manifest model for environment kind, Python path, ComfyUI root, Python version, platform, source revisions, dependency record paths, and prepared timestamp.
- [x] 1.2 Add helpers for resolving safe workspace paths for `/workspace/.venv` and `/workspace/.luma-forge` metadata.
- [x] 1.3 Add tests for manifest serialization, required fields, safe path handling, and secret-safe metadata output.

## 2. Provisioner Volume Environment

- [x] 2.1 Replace container-local dependency installation with creation or reuse of a volume-local virtual environment under the mounted workspace.
- [x] 2.2 Install ComfyUI requirements through the volume-local virtual environment interpreter.
- [x] 2.3 Install Custom Node requirements through the same volume-local virtual environment interpreter.
- [x] 2.4 Write pip install reports and `pip freeze` records under workspace metadata after dependency installation.
- [x] 2.5 Write the runtime manifest only after ComfyUI, Custom Nodes, model assets, dependency installation, and final validation succeed.
- [x] 2.6 Update provisioner progress messages and structured error mapping for virtual environment creation, dependency installation, and metadata writing failures.

## 3. Provisioner Validation

- [x] 3.1 Validate the volume-local Python interpreter exists before reporting provisioning success.
- [x] 3.2 Validate runtime metadata exists and matches the expected prepared environment shape before reporting provisioning success.
- [x] 3.3 Validate dependency record files exist when dependency installation completed.
- [x] 3.4 Add provisioner tests proving ComfyUI and Custom Node requirements are installed through `/workspace/.venv/bin/python`, not the container Python.
- [x] 3.5 Add provisioner tests for missing venv, missing manifest, failed venv creation, failed dependency install, and cancelled preparation.

## 4. Endpoint Runtime Use

- [x] 4.1 Add endpoint-side prepared runtime manifest loading and compatibility validation.
- [x] 4.2 Require endpoint runtime metadata to describe a volume-local virtual environment before generation.
- [x] 4.3 Start ComfyUI with the volume-local Python interpreter declared by the runtime manifest.
- [x] 4.4 Ensure endpoint startup does not clone repositories, download models, run pip, or repair the prepared volume.
- [x] 4.5 Add endpoint tests for invalid environment kind, missing manifest, missing venv interpreter, missing ComfyUI entrypoint, and successful startup command construction.

## 5. Shared Worker Images

- [x] 5.1 Define a shared provider-neutral worker Docker base.
- [x] 5.2 Update provisioner and endpoint image build structure so both images use the shared worker base.
- [x] 5.3 Update worker deployment automation to validate and publish provisioner and endpoint images through separate workflows from the shared worker Dockerfile.
- [x] 5.4 Update image tagging and documentation so operators can select and roll back worker image refs.

## 6. Documentation and Specs

- [x] 6.1 Update README and worker docs to describe the network-volume virtual environment layout.
- [x] 6.2 Update workspace provisioning flow docs to say readiness validates the prepared volume runtime but does not run generation.
- [x] 6.3 Update endpoint worker docs to state that ComfyUI runs through the volume-local venv.
- [x] 6.4 Update provisioner worker docs to state that ComfyUI and Custom Node Python dependencies must not install into the ephemeral container environment.

## 7. Verification

- [x] 7.1 Run the Provisioner Worker test suite.
- [x] 7.2 Run the RunPod Endpoint Worker test suite.
- [x] 7.3 Build the provisioner and endpoint worker container images from the shared worker Dockerfile.
- [x] 7.4 Run native backend verification if native build configuration or workspace provisioning code changes.
- [x] 7.5 Run `openspec validate install-runtime-deps-in-volume-venv` and confirm the change is apply-ready.
