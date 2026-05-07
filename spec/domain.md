# Domain Model

## Domain

LumaForge is a local desktop application that helps a user provision GPU cloud infrastructure and run ComfyUI workflows remotely.

LumaForge does not perform ML inference locally. The user works with a local UI, while inference runs on remote infrastructure provided by a GPU Cloud Provider.

In v1, LumaForge supports only RunPod as a GPU Cloud Provider.

LumaForge does not operate its own backend service. The desktop application communicates directly with RunPod from the Rust/Tauri layer.

This document focuses on provisioning and Workspace management. Sessions, generation history, prompts, requests, generated images, and result management are outside the main scope of this document, except for cleanup boundaries.

## Ubiquitous Language

- [GPU Cloud Provider](./ubiquitous-language/gpu-cloud-provider.md)
- [Provider API Key](./ubiquitous-language/provider-api-key.md)
- [Provider Resource](./ubiquitous-language/provider-resource.md)
- [Placement Plan](./ubiquitous-language/placement-plan.md)
- [Persistent Storage Volume](./ubiquitous-language/persistent-storage-volume.md)
- [Provisioning Pod](./ubiquitous-language/provisioning-pod.md)
- [Serverless Endpoint](./ubiquitous-language/serverless-endpoint.md)
- [Workflow](./ubiquitous-language/workflow.md)
- [Custom Nodes](./ubiquitous-language/custom-nodes.md)
- [Workflow Preset](./ubiquitous-language/workflow-preset.md)
- [Workflow Catalog](./ubiquitous-language/workflow-catalog.md)
- [Provisioning Profile](./ubiquitous-language/provisioning-profile.md)
- [Endpoint Profile](./ubiquitous-language/endpoint-profile.md)
- [Provisioner Worker](./ubiquitous-language/provisioner-worker.md)
- [Endpoint Worker](./ubiquitous-language/endpoint-worker.md)
- [Workspace](./ubiquitous-language/workspace.md)
- [Workspace Catalog](./ubiquitous-language/workspace-catalog.md)
- [Workspace Provisioning Progress](./ubiquitous-language/workspace-provisioning-progress.md)
- [Workspace Resource Cleanup](./ubiquitous-language/workspace-resource-cleanup.md)
- [Factory Reset](./ubiquitous-language/factory-reset.md)
- [Health Check](./ubiquitous-language/health-check.md)

## Flows

- [GPU Cloud Provider Setup](./flows/gpu-cloud-provider-setup.md)
- [Workspace Setup](./flows/workspace-setup.md)
- [Workspace Provisioning](./flows/workspace-provisioning.md)
- [Remove Workspace](./flows/remove-workspace.md)
- [System Rollback](./flows/system-rollback.md)
