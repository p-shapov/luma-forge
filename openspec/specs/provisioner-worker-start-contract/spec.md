# Provisioner Worker Start Contract Specification

## Purpose
Define the minimal start request contract shared by Native Workspace Provisioning and the Provisioner Worker.

## Requirements
### Requirement: Minimal worker start request
The Provisioner Worker start request contract SHALL contain only the data required for the worker to correlate a job and prepare declared model assets on the mounted workspace.

#### Scenario: Start request contains model asset preparation input
- **WHEN** Native Workspace Provisioning starts an idle Provisioner Worker job
- **THEN** the `POST /start` request SHALL include the active Workspace identifier as `job_id`
- **AND** the request SHALL include declared model assets with asset id, display name, Hugging Face source coordinates, and ComfyUI-relative install path
- **AND** the request MUST NOT include Workflow Preset id, Workflow Preset version, workflow execution type, required base volume size, runtime contract reference, provisioner contract reference, resolved runtime image snapshot, resolved provisioner image snapshot, endpoint image fields, runtime manifest paths, endpoint runtime paths, Provider API Keys, or worker bearer tokens inside the JSON body

#### Scenario: Worker rejects unsupported start fields
- **WHEN** a `POST /start` request contains fields outside the minimal worker start request contract
- **THEN** the Provisioner Worker SHALL reject the request as an invalid request
- **AND** the Provisioner Worker MUST NOT start a job or mutate the mounted workspace
