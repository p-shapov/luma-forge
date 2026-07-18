# Native Backend

The native backend is LumaForge's authoritative local API. It owns durable state, credentials, provider access, validation, and long-running Runtime operations.

## Core Concepts

LumaForge creates a Workspace for a bundled Workflow revision, attaches a remote GPU environment to it as a Runtime, and records each provision or cleanup attempt as a RuntimeOperation.

- **Workspace** is a persisted local record for one exact Workflow revision. It exists before provisioning, can have at most one Runtime attached, and remains after that Runtime is cleaned up.
- **Workflow revision** is an immutable, versioned workflow definition. It pins the ComfyUI graph, required models, execution contract, runtime preset, and worker contracts.
- **Runtime** is the remote GPU environment attached to a Workspace and used to execute its workflow. It combines provider-neutral lifecycle state (`provisioning`, `ready`, `cleaning_up`, or `failed`) with provider-specific configuration and resource details.
- **RuntimeOperation** is the durable record of one background provision or cleanup attempt. It tracks progress, outcome, timestamps, and optional trace correlation, and remains available after Runtime cleanup.

## Runtime Lifecycle

A typical provision flow is:

```text
commands.createWorkspace
  → commands.provisionWorkspace
  → events.workspaceChanged / events.runtimeOperation
  → ready | failed
```

Provision and cleanup return their initial Workspace and operation snapshots immediately. Provider work continues in the background and can be observed through events or `commands.getRuntimeOperations`.

## Frontend Contract

Use the [generated TypeScript bindings](../src/generated/commands.ts) for commands, events, requests, responses, and error types. Tauri Specta generates this file from `facade/`.

After changing a command, event, or exported DTO, regenerate the bindings from the repository root:

```bash
bun run codegen:commands
```

Application results use a typed success or error envelope:

```typescript
export interface CommandError<Code> {
  code: Code;
  traceId: string;
}

type CommandResult<Data, Code>
  = | { status: "ok"; data: Data }
    | { status: "error"; error: CommandError<Code> };
```

Use `error.traceId` to find matching diagnostics records.

### Commands

Each entry uses the generated TypeScript method name. The underlying Tauri command uses the corresponding `snake_case` name defined in `facade/commands.rs`.

#### Workflows and Workspaces

##### `commands.getWorkflows`

Lists bundled workflow revisions available for new Workspaces.

**Input**

```typescript
{
  offset: number;
  limit: number; // 1..100
}
```

**Output data**

```typescript
{
  workflows: WorkflowDto[];
  total: number;
}
```

##### `commands.getWorkspaces`

Lists Workspace snapshots, including any attached Runtime.

**Input**

```typescript
{
  offset: number;
  limit: number; // 1..100
}
```

**Output data**

```typescript
{
  workspaces: WorkspaceDto[];
  total: number;
}
```

##### `commands.createWorkspace`

Creates a Workspace pinned to an exact workflow revision.

**Input**

```typescript
{
  workflow: {
    id: string;
    revision: string;
  };
}
```

**Output data:** `WorkspaceDto`

##### `commands.deleteWorkspace`

Deletes a Workspace with no attached Runtime or running operation.

**Input**

```typescript
{
  workspaceId: string;
}
```

**Output data:** `null`

#### Runtime Lifecycle

##### `commands.provisionWorkspace`

Starts Runtime provisioning and returns the initial snapshots.

**Input**

```typescript
{
  workspaceId: string;
  runtime: {
    runtimeKind: "runpod";
    datacenterId: string;
    gpuId: string;
    volumeSizeGb: number;
  };
}
```

**Output data:** `WorkspaceOperationDto`

##### `commands.cleanupWorkspace`

Starts cleanup for an attached `ready` or `failed` Runtime.

**Input**

```typescript
{
  workspaceId: string;
}
```

**Output data:** `WorkspaceOperationDto`

##### `commands.getRuntimeOperations`

Lists operation history globally or for one Workspace.

**Input**

```typescript
{
  workspaceId: string | null;
  offset: number;
  limit: number; // 1..100
}
```

**Output data**

```typescript
{
  operations: RuntimeOperationDto[];
  total: number;
}
```

##### `commands.getRunpodPlacement`

Returns available RunPod datacenters, GPUs, and maximum volume size.

**Input:** none

**Output data:** `RunpodPlacementDto`

#### Credentials

The RunPod and Hugging Face variants use the same request and response shapes.

##### `commands.setupRunpodApiKey` / `commands.setupHuggingFaceApiKey`

Validates and stores a provider API key, then returns safe identity metadata.

**Input**

```typescript
{
  apiKey: string;
}
```

**Output data:** `IdentityDto`

##### `commands.getRunpodIdentity` / `commands.getHuggingFaceIdentity`

Validates the stored provider key and returns safe identity metadata.

**Input:** none

**Output data:** `IdentityDto`

##### `commands.deleteRunpodApiKey` / `commands.deleteHuggingFaceApiKey`

Deletes the provider key from the OS keyring.

**Input:** none

**Output data:** `null`

### Shared Response Shapes

These generated shapes are reused by multiple command outputs:

```typescript
interface WorkflowDto {
  id: string;
  revision: string;
  name: string;
  description: string;
  requiredVolumeSizeGb: number;
  requiresHuggingFaceApiKey: boolean;
}

interface WorkspaceDto {
  id: string;
  workflow: {
    id: string;
    revision: string;
  };
  createdAt: string;
  runtime: RuntimeDto | null;
}

interface RuntimeDto {
  state: "provisioning" | "ready" | "cleaning_up" | "failed";
  provider: {
    runtimeKind: "runpod";
    datacenterId: string;
    gpuId: string;
    volumeSizeGb: number;
  };
}

interface WorkspaceOperationDto {
  workspace: WorkspaceDto;
  operation: RuntimeOperationDto;
}

interface RuntimeOperationDto {
  id: string;
  workspaceId: string;
  runtimeKind: "runpod";
  kind: "provision" | "cleanup";
  state: "running" | "succeeded" | "failed";
  traceId: string | null;
  progress: RuntimeProgressDto;
  createdAt: string;
  updatedAt: string;
  finishedAt: string | null;
}

type RuntimeProgressDto
  = | {
    progressKind: "runpod_provision";
    step:
      | "create_network_volume"
      | "start_provisioner_pod"
      | "poll_provisioner"
      | "terminate_provisioner_pod"
      | "create_template"
      | "create_endpoint";
  }
  | {
    progressKind: "runpod_cleanup";
    step:
      | "delete_endpoint"
      | "delete_template"
      | "terminate_provisioner_pod"
      | "delete_network_volume";
  };

interface RunpodPlacementDto {
  maxVolumeSizeGb: number;
  datacenters: Array<{
    id: string;
    name: string;
    gpus: Array<{
      id: string;
      name: string;
      vramGb: number;
    }>;
  }>;
}

interface IdentityDto {
  keyName: string | null;
  username: string | null;
  email: string | null;
}
```

### Events

Events are emitted only after the corresponding state is durably persisted.

| Wire event          | Generated listener        | Emitted after                                                                        |
| ------------------- | ------------------------- | ------------------------------------------------------------------------------------ |
| `workspace_changed` | `events.workspaceChanged` | Workspace creation or a Runtime transition. Carries the complete Workspace snapshot. |
| `workspace_deleted` | `events.workspaceDeleted` | Workspace deletion. Carries the Workspace ID.                                        |
| `runtime_operation` | `events.runtimeOperation` | An operation state or progress transition. Carries the complete operation snapshot.  |

## Local State and Diagnostics

Native support files are configured in `src/lib.rs` and stored under the Tauri `app_data_dir()`. On macOS:

```text
~/Library/Application Support/com.luma-forge/
```

| File              | Contents                                                           |
| ----------------- | ------------------------------------------------------------------ |
| `db.sqlite`       | Workspaces, attached Runtime state, and Runtime operation history. |
| `diagnostics.log` | JSON diagnostics records produced by `#[diagnostic]`.              |

### Resetting Local State

During pre-v1 development, schema bootstrap may reject a database created by an older build. Stop the app before deleting `db.sqlite`.

> Deleting `db.sqlite` removes local state only. It does not delete remote RunPod volumes, pods, templates, or endpoints.

Manual database deletion is troubleshooting guidance, not a supported migration or downgrade path.

### Finding Failures

Commands create root traces, nested operations share the same trace, and detached Runtime work preserves or restores it. Values are omitted by default and appear only when explicitly marked safe or redacted.

1. Get the trace ID from `CommandError.traceId` or `RuntimeOperationEvent.operation.traceId`.
2. Search `diagnostics.log` for that exact value. If no trace ID is available, search by approximate time, Workspace ID, action, or error text.
3. Follow matching `call.start`, `call.success`, and `call.error` records by function, trace ID, and span ID to locate the failing command, service, adapter, or provider boundary.
