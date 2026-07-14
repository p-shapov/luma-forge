# Runtime Provider Payload Persistence Design

**Status:** Approved for implementation planning

## Goal

Replace provider-specific SQLite extension tables with opaque JSON payloads stored on the existing provider-neutral runtime anchor and operation history rows.

The change must make a future provider persistable without adding SQLite tables, entities, relations, provider-specific SQL, or persistence dispatch while preserving the current one-runtime-per-workspace admission and atomic transition semantics.

## Relationship To The Existing Runtime Dispatch Design

This design supersedes only the provider-specific persistence decisions in [`2026-07-13-runtime-dispatch-boundary-design.md`](./2026-07-13-runtime-dispatch-boundary-design.md):

- the composite SQLite adapter no longer dispatches to provider-specific SQL modules;
- provider-specific runtime state and operation progress no longer live in provider-specific extension tables;
- workspace and operation hydration no longer issue provider-specific follow-up queries.

The following approved decisions remain unchanged:

- `RuntimeService` owns closed application lifecycle dispatch;
- `workspace_runtimes` is the one-runtime-per-workspace anchor and admission lock;
- `runtime_operations` is durable operation history;
- runtime transitions save the workspace runtime and operation atomically;
- events are emitted only after the transaction commits;
- the facade owns transport mapping rather than lifecycle routing;
- runtime providers remain a closed, compiler-checked set rather than a dynamic plugin registry.

## Context

The current SQLite model separates provider-neutral and RunPod-specific values:

- `workspace_runtimes` stores `workspace_id`, `runtime_kind`, and lifecycle `state`;
- `runpod_workspace_runtimes` stores RunPod configuration and remote resource IDs;
- `runtime_operations` stores provider-neutral operation metadata;
- `runpod_runtime_operation_progress` stores the RunPod lifecycle step.

This preserves relational provider fields, but adding another provider requires two new tables, SeaORM entities, relations, provider-specific SQL mapping, persistence dispatcher arms, and additional hydration queries.

Provider-specific runtime fields are not expected to participate in SQL filtering, ordering, joins, or relational constraints. Planned queries use provider-neutral fields such as workspace ID, runtime kind, runtime state, operation kind, operation state, and timestamps. Provider-specific state is always loaded and saved as a complete typed value.

Under those requirements, provider-specific relations add cost without providing a query capability the application plans to use.

## Decision

Provider-specific runtime state and operation progress are serialized as tagged JSON and stored inline on their provider-neutral rows:

- `workspace_runtimes.provider_payload` stores `RuntimeProvider`;
- `runtime_operations.progress_payload` stores `RuntimeProgress`.

The persisted JSON contract is the pinned Serde representation of the typed application enums and their provider-specific models. SQLite repositories use generic `serde_json::to_string` and `serde_json::from_str`; they contain no provider-specific payload DTO, provider match, or provider persistence module.

The JSON remains opaque to generic SQL. Application and repository port signatures remain typed and do not expose `serde_json::Value`.

## Target Schema

### `workspace_runtimes`

| Column | Type | Contract |
| --- | --- | --- |
| `workspace_id` | `TEXT` | Primary key and foreign key to `workspaces.id`; enforces at most one runtime per workspace |
| `runtime_kind` | `TEXT` | Provider-neutral discriminator used by admission and future filtering |
| `state` | `TEXT` | Provider-neutral lifecycle state used by admission and filtering |
| `provider_payload` | `TEXT` | Non-null tagged JSON representation of `RuntimeProvider`; opaque to SQL |

The foreign key keeps the current cascade behavior when its workspace is removed. The anchor row remains the source of truth for runtime attachment and lifecycle admission.

### `runtime_operations`

| Column | Type | Contract |
| --- | --- | --- |
| `id` | `TEXT` | Operation UUID primary key |
| `workspace_id` | `TEXT` | Stable workspace correlation retained with history |
| `runtime_kind` | `TEXT` | Provider-neutral discriminator used by recovery and filtering |
| `operation_kind` | `TEXT` | Provider-neutral `provision` or `cleanup` discriminator |
| `state` | `TEXT` | Provider-neutral `running`, `succeeded`, or `failed` state |
| `trace_id` | `TEXT NULL` | Optional diagnostics correlation |
| `progress_payload` | `TEXT` | Non-null tagged JSON representation of `RuntimeProgress`; opaque to SQL |
| `created_at` | `DATETIME` | Stable creation time |
| `updated_at` | `DATETIME` | Last durable transition time |
| `finished_at` | `DATETIME NULL` | Terminal transition time |

Operation rows continue to survive workspace and runtime cleanup so history and progress remain readable after the runtime anchor is deleted.

### Removed Schema

The current schema removes these tables and their SeaORM entities and relations:

- `runpod_workspace_runtimes`;
- `runpod_runtime_operation_progress`.

No generic payload child tables replace them.

## Neutral Columns And JSON Tags

The neutral columns remain authoritative for SQL queries, admission, ordering, and recovery selection. The tagged JSON representation intentionally repeats only the provider discriminator and, for progress, its operation family.

This limited duplication is required to deserialize the application enums generically without provider dispatch in SQLite adapters. Every read and write cross-checks the JSON enum variant against the neutral columns. A mismatch is corrupt data rather than a source-of-truth choice.

Lifecycle state and operation state are not repeated inside the provider payloads.

## Serialization Contract

The persisted application types derive `Serialize` and `Deserialize` with explicit, stable Serde names. The representation must not rely on Rust variant or field names implicitly.

`RuntimeProvider` uses a tagged representation equivalent to:

```json
{
  "provider": "runpod",
  "payload": {
    "config": {
      "datacenter_id": "EU-RO-1",
      "gpu_id": "gpu-1",
      "volume_size_gb": 100
    },
    "resources": {
      "network_volume_id": null,
      "provisioner_pod_id": null,
      "template_id": null,
      "endpoint_id": null
    }
  }
}
```

`RuntimeProgress` uses a tagged representation equivalent to:

```json
{
  "provider": "runpod",
  "payload": {
    "operation": "provision",
    "step": "create_network_volume"
  }
}
```

The contract requires:

- explicit `snake_case` names for providers, operation families, steps, and fields;
- rejection of invalid JSON, unknown fields or enum variants, invalid field types, and unsupported shapes rather than silently ignoring them;
- no persistence version field in the pre-v1 contract;
- no raw secret, API key, bearer token, or credential field in either payload;
- no raw payload in diagnostics, errors, command responses, or generated frontend types.

Serde is a mechanism-neutral serialization dependency. Application models gain no SeaORM, SQLite, Tauri, Specta, keyring, or provider-client dependency.

`RuntimeKind` owns the explicit stable lowercase identifier stored in the neutral `runtime_kind` column. SQLite code uses that provider-neutral conversion and does not add a provider match when the runtime set grows.

## Generic Application Invariants

The closed application enums expose provider-neutral discriminator accessors sufficient for generic validation:

- `RuntimeProvider` identifies its `RuntimeKind`;
- `RuntimeProgress` identifies its `RuntimeKind`;
- `RuntimeProgress` identifies its `RuntimeOperationKind`.

Before persistence, the transition must establish:

- `workspace.id == operation.workspace_id`;
- an attached runtime exists unless this is a successful cleanup terminal transition;
- the attached runtime kind equals `operation.runtime_kind`;
- the progress runtime kind equals `operation.runtime_kind`;
- the progress operation kind equals `operation.kind`.

After hydration, the same discriminator checks run again against the neutral columns. Validation on write prevents invalid application snapshots from becoming durable; validation on read fails closed for malformed, manually modified, or incompatible databases.

These checks belong to the typed application model contract. SQLite adapters call them generically and do not match provider variants.

## Atomic Transition Write Flow

There is one transaction for the complete runtime transition, not one transaction per row.

```text
validate typed workspace + operation invariants
serialize provider_payload + progress_payload
BEGIN
    insert or update runtime_operations, including progress_payload
    insert, claim, update, or delete workspace_runtimes,
        including provider_payload whenever the anchor remains attached
COMMIT
emit application events
```

Serialization occurs before opening the SQLite transaction. The transaction retains the current guarantees:

- a new provision operation claims the workspace by inserting its unique runtime anchor;
- cleanup admission conditionally claims an anchor only from `ready` or `failed`;
- a rejected admission rolls back the operation row as well as all anchor changes;
- every non-terminal transition atomically stores neutral state, provider state, operation metadata, and progress;
- successful cleanup deletes the runtime anchor but retains the terminal operation and its progress;
- any persistence failure rolls back the whole transition;
- events are best-effort only after commit and never determine transaction success.

The representation change does not alter remote side-effect ordering, retries, idempotency, or interrupted-operation recovery behavior.

## Hydration And Query Shape

### Workspace Reads

`SqliteWorkspaceRepository::get` and `page` read the runtime anchor together with its workspace. When an anchor exists, the repository deserializes `provider_payload` directly into `RuntimeProvider` and combines it with the neutral `state`.

The current provider-specific lookup and bulk hydration pass disappear. A page consists of the existing count query plus the workspace/anchor page query; it does not issue another query per provider.

### Operation Reads

`SqliteRuntimeOperationRepository::page` and `running` deserialize `progress_payload` directly from each selected operation row.

The current provider grouping and provider-specific progress query disappear. Operation filtering, ordering, totals, recovery lookup, and history retention continue using neutral columns.

### Failure Semantics

One malformed payload fails the repository call with `CorruptData`. The repository does not omit the row, substitute an empty payload, infer missing provider state, or fall back to another schema.

## Error Mapping And Diagnostics

- SQLite connection, statement, transaction, or commit failures map to the existing `Unavailable` category.
- Serialization, deserialization, invalid enum discriminators, and neutral/payload mismatches map to the appropriate existing `CorruptData` category.
- Admission conflicts, not-found cases, and rollback behavior retain their current error categories.
- Adapter diagnostics may record the safe error category and source class, but never the raw JSON payload.

No new public application, facade, command, or generated frontend error type is introduced.

## Adding A Future Provider

A future provider still adds its real vertical behavior:

- application runtime model, lifecycle service, and provider ports;
- provider client and adapters;
- credential and catalog handling where required;
- variants in the closed lifecycle and facade command enums;
- explicit application lifecycle dispatch;
- tests for its lifecycle and external boundaries.

For persistence it adds only typed, explicitly named Serde variants and generic discriminator behavior to the application enums. It does not add:

- SQLite tables or columns;
- SeaORM entities or relations;
- provider-specific SQL;
- persistence adapter modules;
- persistence dispatcher arms;
- provider-specific hydration queries.

The SQLite repositories serialize and deserialize the extended closed enums without provider-specific changes.

## Schema Cutover

LumaForge is pre-v1, and existing local development databases do not need to survive this schema change.

The implementation updates the current schema directly and assumes a fresh `db.sqlite`. Existing databases must be deleted and recreated. There is no migration, data copy, dual-read, legacy fallback, or compatibility shim for provider extension tables.

Payload versioning is deferred until the first real requirement to preserve provider payloads across an incompatible model change. Until then, an incompatible payload is `CorruptData`.

## Rejected Alternatives

### Provider-Specific Adapter DTO And Dispatch

This would keep application models independent from their persisted JSON representation, but every provider would still add adapter DTOs and dispatch arms. That is the extension cost this design is removing.

### Generic Payload Child Tables

Generic runtime and progress payload tables would remove provider-specific schema but retain extra relations, orphan checks, and hydration queries. With no payload filtering or joining requirement, inline columns are simpler.

### Opaque `serde_json::Value` In Application Models

Passing opaque JSON through repository ports would remove typed provider state from the application boundary and move validation into lifecycle services. Provider config, resource IDs, cleanup decisions, and progress remain application concepts and must stay typed.

### Full Aggregate JSON Snapshot

Serializing neutral runtime or operation state into the payload would either remove queryable admission fields or duplicate their source of truth. Neutral lifecycle and history columns remain relational.

### Validation Only On Write

Write validation alone cannot protect hydration from malformed files, manual modification, incompatible binaries, or corrupted local state. Read validation is generic and does not require provider dispatch.

## Non-Goals

- Adding a second runtime provider.
- Dynamic provider registration, factories, or plugin loading.
- Changing application lifecycle dispatch.
- Changing repository port signatures.
- Changing Tauri commands, DTOs, events, generated bindings, or frontend behavior.
- Querying, indexing, joining, or partially updating fields inside JSON payloads.
- Migrating or preserving an existing local database.
- Supporting multiple payload versions.
- Changing remote resource recovery, retry, cancellation, timeout, or idempotency behavior.
- Adding query-count tests, SQL-text tests, or assertions whose only purpose is proving removed tables are absent.

## Behavioral Verification

The implementation must demonstrate:

- a runtime containing partial RunPod resource IDs survives `save_transition` followed by workspace `get` and `page` without data loss;
- provision and cleanup progress survive `save_transition` followed by operation `page` and `running` reads;
- malformed JSON returns `CorruptData`;
- a payload provider tag that disagrees with `runtime_kind` returns `CorruptData`;
- a progress operation family that disagrees with `operation_kind` returns `CorruptData` and cannot be persisted;
- a rejected provision or cleanup admission leaves neither a rejected operation nor a partial anchor update;
- a persistence failure rolls back both the operation and runtime anchor changes;
- successful cleanup removes the runtime anchor while preserving terminal operation progress;
- workspace and operation pagination, ordering, totals, neutral filtering, and recovery behavior remain unchanged;
- raw payloads and credentials do not appear in diagnostics or public DTOs.

Tests protect behavior rather than exact JSON strings, generated SQL, query counts, or removed implementation structure.

Required repository checks:

```text
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```
