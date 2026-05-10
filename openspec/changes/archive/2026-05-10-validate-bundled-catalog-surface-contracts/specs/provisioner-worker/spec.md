## MODIFIED Requirements

### Requirement: Prepare Custom Nodes

The Provisioner Worker SHALL install required Custom Nodes declared by the selected Workflow Preset.

#### Scenario: Preset declares Custom Nodes

- **WHEN** the selected Workflow Preset includes required Custom Nodes
- **THEN** the Provisioner Worker SHALL clone each Custom Node from its declared Git source into its declared safe checkout path under the prepared ComfyUI `custom_nodes` directory
- **AND** the Provisioner Worker SHALL install Custom Node dependencies from each declared requirements path when present
- **AND** each requirements path SHALL be resolved relative to its Custom Node checkout root
- **AND** `GET /status` SHALL report an installing Custom Nodes phase while this work is active

#### Scenario: Preset declares no Custom Nodes

- **WHEN** the selected Workflow Preset has an empty required Custom Nodes list
- **THEN** the Provisioner Worker SHALL skip Custom Node installation
- **AND** the provisioning job SHALL continue to the next required preparation step

## ADDED Requirements

### Requirement: Validate Custom Node paths before filesystem writes

The Provisioner Worker SHALL validate Custom Node checkout and requirements paths from the selected Workflow Preset before performing related remote filesystem writes or dependency installation.

#### Scenario: Custom Node checkout path is safe

- **WHEN** a selected Workflow Preset declares a Custom Node checkout path that is relative, normalized, free of current-directory, empty, absolute, and parent-traversal segments, and resolves under the prepared ComfyUI `custom_nodes` directory
- **THEN** the Provisioner Worker MAY clone that Custom Node into the resolved checkout path

#### Scenario: Custom Node checkout path is unsafe

- **WHEN** a selected Workflow Preset declares a Custom Node checkout path that is blank, absolute, contains current-directory, empty, or parent-traversal segments, resolves outside the prepared ComfyUI root, or does not resolve under the prepared ComfyUI `custom_nodes` directory
- **THEN** the Provisioner Worker SHALL reject the start request or fail the active job before cloning the Custom Node
- **AND** the Provisioner Worker MUST NOT write outside the prepared ComfyUI `custom_nodes` directory for that Custom Node

#### Scenario: Custom Node requirements path is absent

- **WHEN** a selected Workflow Preset declares no requirements path for a Custom Node
- **THEN** the Provisioner Worker SHALL skip requirements installation for that Custom Node
- **AND** the Provisioner Worker SHALL continue provisioning when all other Custom Node data is valid

#### Scenario: Custom Node requirements path is safe

- **WHEN** a selected Workflow Preset declares a Custom Node requirements path that is relative, normalized, free of current-directory, empty, absolute, and parent-traversal segments, and resolves under that Custom Node checkout root
- **THEN** the Provisioner Worker MAY install dependencies from that requirements path

#### Scenario: Custom Node requirements path is unsafe

- **WHEN** a selected Workflow Preset declares a Custom Node requirements path that is blank, absolute, contains current-directory, empty, or parent-traversal segments, or resolves outside that Custom Node checkout root
- **THEN** the Provisioner Worker SHALL reject the start request or fail the active job before installing dependencies from that path
- **AND** the Provisioner Worker MUST NOT read requirements files outside the Custom Node checkout root
