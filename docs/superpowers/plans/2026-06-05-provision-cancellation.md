# Provision Cancellation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement user-triggered remote provisioning cancellation as a one-action-per-call reverse state machine in the active native backend.

**Architecture:** Keep cancellation inside `RemoteWorkspaceService`, alongside the existing provisioning and cleanup workflows. A cancel request only marks an in-progress workspace as `Cancelling { phase: Some(current_phase) }`; repeated sync calls then delete the highest-order known resource, roll the phase backward, and return. Successful cancellation clears remote snapshots and resets provisioning to `NotStarted`.

**Tech Stack:** Rust, Tauri native backend, existing `RemoteWorkspaceService` unit tests, existing provider trait cleanup primitives.

---

## File Structure

- Modify `src-tauri/src/domain/workspace.rs`
  - Add a UI-safe cancellation failure variant to `RemoteProvisioningError`.
- Modify `src-tauri/src/remote_workspace/service.rs`
  - Add a `cancel_workspace` service method that marks `InProgress` workspaces as `Cancelling`.
  - Replace the current `Cancelling` skeleton branch in `provision_workspace` with one-step reverse cleanup.
  - Add focused unit tests using the existing fake provider.
- No changes to `src-tauri/src/remote_workspace/provider.rs`
  - Existing `delete_endpoint`, `terminate_provisioner`, and `delete_volume` primitives are enough.
- No frontend or generated command work in this plan.
  - `cancel_workspace_provisioning` is still a refactor placeholder; real command wiring should be planned when workspace persistence/commands are implemented.

---

### Task 1: Add Cancellation Failure Domain Variant

**Files:**
- Modify: `src-tauri/src/domain/workspace.rs`
- Test: `src-tauri/src/remote_workspace/service.rs`

- [ ] **Step 1: Write the failing domain variant test**

In `src-tauri/src/remote_workspace/service.rs`, inside `#[cfg(test)] mod tests`, add this test near the other provisioning status tests:

```rust
#[test]
fn remote_provisioning_error_has_cancellation_cleanup_failed_variant() {
    assert_eq!(
        RemoteProvisioningError::CancellationCleanupFailed,
        RemoteProvisioningError::CancellationCleanupFailed
    );
}
```

- [ ] **Step 2: Run the focused failing test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml remote_provisioning_error_has_cancellation_cleanup_failed_variant
```

Expected: FAIL with an error like:

```text
no variant or associated item named `CancellationCleanupFailed` found for enum `RemoteProvisioningError`
```

- [ ] **Step 3: Add the enum variant**

In `src-tauri/src/domain/workspace.rs`, update `RemoteProvisioningError`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteProvisioningError {
    Provider(ProviderError),
    ProvisionerWorkerTokenMissing,
    ProvisionerWorkerTokenInvalid,
    ProvisionerWorkerUnauthorized,
    ProvisionerWorkerUnavailable,
    ProvisionerWorkerConflict,
    ProvisionerWorkerResponseInvalid,
    ProvisionerWorkerFailed,
    ProvisionerWorkerAssetDownloadFailed,
    ProvisionerWorkerAssetAuthRequired,
    ProvisionerWorkerPathValidationFailed,
    ProvisionerWorkerStepTimeout,
    ProvisionerWorkerUnexpectedError,
    CancellationCleanupFailed,
    InvalidProvisioningState { message: String },
}
```

- [ ] **Step 4: Run the focused test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml remote_provisioning_error_has_cancellation_cleanup_failed_variant
```

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
git add src-tauri/src/domain/workspace.rs src-tauri/src/remote_workspace/service.rs
git commit -m "feat(remote-workspace): add cancellation cleanup failure"
```

---

### Task 2: Mark In-Progress Workspaces As Cancelling

**Files:**
- Modify: `src-tauri/src/remote_workspace/service.rs`

- [ ] **Step 1: Write the failing cancel-initiation test**

In `src-tauri/src/remote_workspace/service.rs`, add this test near the other service tests:

```rust
#[test]
fn cancel_workspace_marks_in_progress_workspace_as_cancelling_without_provider_calls() {
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
        id: "volume".to_string(),
    });
    remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
        phase: RemoteProvisioningPhase::StartingRemoteProvisioner,
    };
    remote.remote_provisioning.percent = Some(25);

    let cancelled = service
        .cancel_workspace(&workspace)
        .expect("in-progress workspace should enter cancellation");

    let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::Cancelling {
            phase: Some(RemoteProvisioningPhase::StartingRemoteProvisioner)
        }
    );
    assert_eq!(remote.remote_provisioning.percent, Some(25));
    assert_eq!(
        remote.remote_resources.remote_volume,
        Some(RemoteVolumeSnapshot {
            id: "volume".to_string(),
        })
    );
    assert!(state.lock().expect("state lock should succeed").calls.is_empty());
}
```

- [ ] **Step 2: Run the focused failing test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml cancel_workspace_marks_in_progress_workspace_as_cancelling_without_provider_calls
```

Expected: FAIL because `RemoteWorkspaceService::cancel_workspace` does not exist.

- [ ] **Step 3: Add the `cancel_workspace` method**

In `src-tauri/src/remote_workspace/service.rs`, add this method inside `impl RemoteWorkspaceService`, after `provision_workspace` and before `execute_workspace`:

```rust
    pub fn cancel_workspace(&self, workspace: &Workspace) -> Result<Workspace, RemoteWorkspaceError> {
        let remote = remote_workspace(workspace)?;

        let RemoteProvisioningStatus::InProgress { phase } = &remote.remote_provisioning.status
        else {
            return Ok(failed_provisioning_workspace(
                workspace,
                None,
                RemoteProvisioningError::InvalidProvisioningState {
                    message: "only in-progress provisioning can be cancelled".to_string(),
                },
            ));
        };

        let mut workspace = workspace.clone();
        let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
        remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
            phase: Some(phase.clone()),
        };
        Ok(workspace)
    }
```

Keep `percent` and resource snapshots unchanged.

- [ ] **Step 4: Run the focused test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml cancel_workspace_marks_in_progress_workspace_as_cancelling_without_provider_calls
```

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
git add src-tauri/src/remote_workspace/service.rs
git commit -m "feat(remote-workspace): mark provisioning as cancelling"
```

---

### Task 3: Reject Cancel From Non-In-Progress States

**Files:**
- Modify: `src-tauri/src/remote_workspace/service.rs`

- [ ] **Step 1: Write non-in-progress cancellation tests**

In `src-tauri/src/remote_workspace/service.rs`, add these tests:

```rust
#[test]
fn cancel_workspace_not_started_marks_invalid_state_without_provider_calls() {
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let service = service_with_state(Arc::clone(&state));
    let workspace = draft_workspace(&service);

    let cancelled = service
        .cancel_workspace(&workspace)
        .expect("invalid cancellation should be represented in workspace state");

    let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::Failed {
            phase: None,
            error: RemoteProvisioningError::InvalidProvisioningState {
                message: "only in-progress provisioning can be cancelled".to_string(),
            },
        }
    );
    assert!(state.lock().expect("state lock should succeed").calls.is_empty());
}

#[test]
fn cancel_workspace_completed_marks_invalid_state_without_provider_calls() {
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_provisioning.status = RemoteProvisioningStatus::Completed;
    remote.remote_provisioning.percent = Some(100);

    let cancelled = service
        .cancel_workspace(&workspace)
        .expect("invalid cancellation should be represented in workspace state");

    let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::Failed {
            phase: None,
            error: RemoteProvisioningError::InvalidProvisioningState {
                message: "only in-progress provisioning can be cancelled".to_string(),
            },
        }
    );
    assert!(state.lock().expect("state lock should succeed").calls.is_empty());
}

#[test]
fn cancel_workspace_failed_marks_invalid_state_without_provider_calls() {
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_provisioning.status = RemoteProvisioningStatus::Failed {
        phase: Some(RemoteProvisioningPhase::CreatingRemoteVolume),
        error: RemoteProvisioningError::Provider(ProviderError::RequestFailed {
            message: "provider request failed".to_string(),
        }),
    };

    let cancelled = service
        .cancel_workspace(&workspace)
        .expect("invalid cancellation should be represented in workspace state");

    let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::Failed {
            phase: None,
            error: RemoteProvisioningError::InvalidProvisioningState {
                message: "only in-progress provisioning can be cancelled".to_string(),
            },
        }
    );
    assert!(state.lock().expect("state lock should succeed").calls.is_empty());
}
```

- [ ] **Step 2: Run the focused tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml cancel_workspace_
```

Expected: PASS for all `cancel_workspace_*` tests.

- [ ] **Step 3: Commit**

Run:

```bash
git add src-tauri/src/remote_workspace/service.rs
git commit -m "test(remote-workspace): reject invalid cancellation states"
```

---

### Task 4: Delete Endpoint During Cancellation

**Files:**
- Modify: `src-tauri/src/remote_workspace/service.rs`

- [ ] **Step 1: Write the failing endpoint rollback test**

In `src-tauri/src/remote_workspace/service.rs`, add:

```rust
#[test]
fn provision_workspace_cancelling_deletes_endpoint_only_and_rolls_back_phase() {
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = workspace_with_all_remote_resources(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
        phase: Some(RemoteProvisioningPhase::CreatingRemoteEndpoint),
    };
    remote.remote_provisioning.percent = Some(75);

    let cancelled = block_on(service.provision_workspace(&workspace))
        .expect("cancellation should delete endpoint");

    let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
    assert_eq!(remote.remote_resources.remote_endpoint, None);
    assert_eq!(
        remote.remote_resources.remote_provisioner,
        Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        })
    );
    assert_eq!(
        remote.remote_resources.remote_volume,
        Some(RemoteVolumeSnapshot {
            id: "volume".to_string(),
        })
    );
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::Cancelling {
            phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::CleaningUp,
            })
        }
    );
    assert_eq!(remote.remote_provisioning.percent, Some(75));
    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec!["delete_endpoint"]
    );
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_cancelling_deletes_endpoint_only_and_rolls_back_phase
```

Expected: FAIL because the current `Cancelling` branch marks invalid state.

- [ ] **Step 3: Add endpoint cancellation branch**

In `src-tauri/src/remote_workspace/service.rs`, replace this existing branch:

```rust
            RemoteProvisioningStatus::Cancelling { phase } => Ok(failed_provisioning_workspace(
                workspace,
                phase.clone(),
                RemoteProvisioningError::InvalidProvisioningState {
                    message: "provisioning cancellation is not implemented in this skeleton"
                        .to_string(),
                },
            )),
```

with:

```rust
            RemoteProvisioningStatus::Cancelling { phase } => {
                cancel_provisioning_step(workspace, remote, provider.as_ref(), phase.clone()).await
            }
```

Then add this helper below `cleanup_workspace` and above `cleanup_failed_workspace`:

```rust
async fn cancel_provisioning_step(
    workspace: &Workspace,
    remote: &RemoteWorkspace,
    provider: &dyn RemoteWorkspaceProvider,
    phase: Option<RemoteProvisioningPhase>,
) -> Result<Workspace, RemoteWorkspaceError> {
    if let Some(endpoint) = remote.remote_resources.remote_endpoint.as_ref() {
        return match ignore_cleanup_error(
            provider
                .delete_endpoint(DeleteEndpointParams {
                    workspace_id: workspace.id.clone(),
                    endpoint_id: endpoint.id.clone(),
                })
                .await,
            RemoteWorkspaceError::RemoteEndpointNotFound,
        ) {
            Ok(()) => Ok(update_cancelling_workspace(
                workspace,
                Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::CleaningUp,
                }),
                75,
                |resources| {
                    resources.remote_endpoint = None;
                },
            )),
            Err(error) => Ok(failed_cancellation_workspace(workspace, phase, error)),
        };
    }

    Ok(update_cancelling_workspace(workspace, None, 0, |_| {}))
}
```

Add these helpers below `failed_provisioning_workspace`:

```rust
fn failed_cancellation_workspace(
    workspace: &Workspace,
    phase: Option<RemoteProvisioningPhase>,
    error: RemoteWorkspaceError,
) -> Workspace {
    let provisioning_error = match error {
        RemoteWorkspaceError::Provider(error) => RemoteProvisioningError::Provider(error),
        _ => RemoteProvisioningError::CancellationCleanupFailed,
    };
    failed_provisioning_workspace(workspace, phase, provisioning_error)
}

fn update_cancelling_workspace(
    workspace: &Workspace,
    phase: Option<RemoteProvisioningPhase>,
    percent: u8,
    update_resources: impl FnOnce(&mut RemoteWorkspaceResources),
) -> Workspace {
    update_provisioning_workspace(
        workspace,
        RemoteProvisioningStatus::Cancelling { phase },
        percent,
        update_resources,
    )
}
```

Make sure `provider.rs` imports include `RemoteWorkspaceProvider`:

```rust
use super::{
    errors::RemoteWorkspaceError,
    provider::{
        CreateEndpointParams, CreateVolumeParams, DeleteEndpointParams, DeleteVolumeParams,
        GetProvisionerStatusParams, RemoteWorkspaceProvider, StartProvisionerParams,
        TerminateProvisionerParams,
    },
    registry::RemoteWorkspaceProviderRegistry,
};
```

- [ ] **Step 4: Run the focused test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_cancelling_deletes_endpoint_only_and_rolls_back_phase
```

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
git add src-tauri/src/remote_workspace/service.rs
git commit -m "feat(remote-workspace): delete endpoint during cancellation"
```

---

### Task 5: Terminate Provisioner During Cancellation

**Files:**
- Modify: `src-tauri/src/remote_workspace/service.rs`

- [ ] **Step 1: Write the failing provisioner rollback test**

In `src-tauri/src/remote_workspace/service.rs`, add:

```rust
#[test]
fn provision_workspace_cancelling_terminates_provisioner_without_polling_status() {
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
        id: "volume".to_string(),
    });
    remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
        id: "provisioner".to_string(),
        status_url: "https://status.example".to_string(),
    });
    remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
        phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
            status: RemoteProvisionerStatus::Running,
        }),
    };
    remote.remote_provisioning.percent = Some(60);

    let cancelled = block_on(service.provision_workspace(&workspace))
        .expect("cancellation should terminate provisioner");

    let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
    assert_eq!(remote.remote_resources.remote_provisioner, None);
    assert_eq!(
        remote.remote_resources.remote_volume,
        Some(RemoteVolumeSnapshot {
            id: "volume".to_string(),
        })
    );
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::Cancelling {
            phase: Some(RemoteProvisioningPhase::StartingRemoteProvisioner)
        }
    );
    assert_eq!(remote.remote_provisioning.percent, Some(25));
    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec!["terminate_provisioner"]
    );
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_cancelling_terminates_provisioner_without_polling_status
```

Expected: FAIL because cancellation currently handles only endpoint deletion.

- [ ] **Step 3: Add provisioner cancellation branch**

In `cancel_provisioning_step`, after the endpoint branch and before the final `Ok(update_cancelling_workspace(...))`, add:

```rust
    if let Some(provisioner) = remote.remote_resources.remote_provisioner.as_ref() {
        return match ignore_cleanup_error(
            provider
                .terminate_provisioner(TerminateProvisionerParams {
                    workspace_id: workspace.id.clone(),
                    provisioner_id: provisioner.id.clone(),
                })
                .await,
            RemoteWorkspaceError::RemoteProvisionerNotFound,
        ) {
            Ok(()) => Ok(update_cancelling_workspace(
                workspace,
                Some(RemoteProvisioningPhase::StartingRemoteProvisioner),
                25,
                |resources| {
                    resources.remote_provisioner = None;
                },
            )),
            Err(error) => Ok(failed_cancellation_workspace(workspace, phase, error)),
        };
    }
```

- [ ] **Step 4: Run the focused test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_cancelling_terminates_provisioner_without_polling_status
```

Expected: PASS. The provider calls must not include `get_provisioner_status`.

- [ ] **Step 5: Commit**

Run:

```bash
git add src-tauri/src/remote_workspace/service.rs
git commit -m "feat(remote-workspace): terminate provisioner during cancellation"
```

---

### Task 6: Delete Volume And Finish Cancellation

**Files:**
- Modify: `src-tauri/src/remote_workspace/service.rs`

- [ ] **Step 1: Write the failing volume rollback test**

In `src-tauri/src/remote_workspace/service.rs`, add:

```rust
#[test]
fn provision_workspace_cancelling_deletes_volume_and_resets_to_not_started() {
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
        id: "volume".to_string(),
    });
    remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
        phase: Some(RemoteProvisioningPhase::StartingRemoteProvisioner),
    };
    remote.remote_provisioning.percent = Some(25);

    let cancelled = block_on(service.provision_workspace(&workspace))
        .expect("cancellation should delete volume");

    let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
    assert_eq!(
        remote.remote_resources,
        RemoteWorkspaceResources {
            remote_volume: None,
            remote_provisioner: None,
            remote_endpoint: None,
        }
    );
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::NotStarted
    );
    assert_eq!(remote.remote_provisioning.percent, None);
    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec!["delete_volume"]
    );
}
```

- [ ] **Step 2: Write the already-empty completion test**

In `src-tauri/src/remote_workspace/service.rs`, add:

```rust
#[test]
fn provision_workspace_cancelling_without_resources_resets_to_not_started_without_provider_calls() {
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
        phase: Some(RemoteProvisioningPhase::CreatingRemoteVolume),
    };
    remote.remote_provisioning.percent = Some(10);

    let cancelled = block_on(service.provision_workspace(&workspace))
        .expect("empty cancellation should reset workspace");

    let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
    assert_eq!(
        remote.remote_resources,
        RemoteWorkspaceResources {
            remote_volume: None,
            remote_provisioner: None,
            remote_endpoint: None,
        }
    );
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::NotStarted
    );
    assert_eq!(remote.remote_provisioning.percent, None);
    assert!(state.lock().expect("state lock should succeed").calls.is_empty());
}
```

- [ ] **Step 3: Run the failing tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_cancelling_
```

Expected: the new volume/empty cancellation tests FAIL until volume deletion and final reset are implemented.

- [ ] **Step 4: Add volume deletion and final reset**

In `cancel_provisioning_step`, after the provisioner branch and before the final `Ok(update_cancelling_workspace(...))`, add:

```rust
    if let Some(volume) = remote.remote_resources.remote_volume.as_ref() {
        return match ignore_cleanup_error(
            provider
                .delete_volume(DeleteVolumeParams {
                    workspace_id: workspace.id.clone(),
                    volume_id: volume.id.clone(),
                })
                .await,
            RemoteWorkspaceError::RemoteVolumeNotFound,
        ) {
            Ok(()) => Ok(reset_cancelled_workspace(workspace)),
            Err(error) => Ok(failed_cancellation_workspace(workspace, phase, error)),
        };
    }
```

Replace the helper's final return:

```rust
    Ok(update_cancelling_workspace(workspace, None, 0, |_| {}))
```

with:

```rust
    Ok(reset_cancelled_workspace(workspace))
```

Add this helper below `update_cancelling_workspace`:

```rust
fn reset_cancelled_workspace(workspace: &Workspace) -> Workspace {
    let mut workspace = workspace.clone();
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_resources = RemoteWorkspaceResources {
        remote_volume: None,
        remote_provisioner: None,
        remote_endpoint: None,
    };
    remote.remote_provisioning.status = RemoteProvisioningStatus::NotStarted;
    remote.remote_provisioning.percent = None;
    workspace
}
```

- [ ] **Step 5: Run the focused tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_cancelling_
```

Expected: PASS for all `provision_workspace_cancelling_*` tests added so far.

- [ ] **Step 6: Commit**

Run:

```bash
git add src-tauri/src/remote_workspace/service.rs
git commit -m "feat(remote-workspace): finish cancellation cleanup"
```

---

### Task 7: Cover Missing Snapshots And Not-Found Cleanup

**Files:**
- Modify: `src-tauri/src/remote_workspace/service.rs`

- [ ] **Step 1: Write missing snapshot rollback test**

In `src-tauri/src/remote_workspace/service.rs`, add:

```rust
#[test]
fn provision_workspace_cancelling_missing_endpoint_skips_to_provisioner_cleanup() {
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
        id: "volume".to_string(),
    });
    remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
        id: "provisioner".to_string(),
        status_url: "https://status.example".to_string(),
    });
    remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
        phase: Some(RemoteProvisioningPhase::CreatingRemoteEndpoint),
    };
    remote.remote_provisioning.percent = Some(75);

    let cancelled = block_on(service.provision_workspace(&workspace))
        .expect("missing endpoint should skip to provisioner cleanup");

    let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
    assert_eq!(remote.remote_resources.remote_endpoint, None);
    assert_eq!(remote.remote_resources.remote_provisioner, None);
    assert_eq!(
        remote.remote_resources.remote_volume,
        Some(RemoteVolumeSnapshot {
            id: "volume".to_string(),
        })
    );
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::Cancelling {
            phase: Some(RemoteProvisioningPhase::StartingRemoteProvisioner)
        }
    );
    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec!["terminate_provisioner"]
    );
}
```

- [ ] **Step 2: Write not-found cleanup test**

In `src-tauri/src/remote_workspace/service.rs`, add:

```rust
#[test]
fn provision_workspace_cancelling_ignores_endpoint_not_found() {
    let state = Arc::new(Mutex::new(ProviderState {
        delete_endpoint_error: Some(RemoteWorkspaceError::RemoteEndpointNotFound),
        ..ProviderState::default()
    }));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = workspace_with_all_remote_resources(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
        phase: Some(RemoteProvisioningPhase::CreatingRemoteEndpoint),
    };
    remote.remote_provisioning.percent = Some(75);

    let cancelled = block_on(service.provision_workspace(&workspace))
        .expect("endpoint not found should be treated as already deleted");

    let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
    assert_eq!(remote.remote_resources.remote_endpoint, None);
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::Cancelling {
            phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::CleaningUp,
            })
        }
    );
    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec!["delete_endpoint"]
    );
}
```

- [ ] **Step 3: Run the focused tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_cancelling_missing_endpoint_skips_to_provisioner_cleanup
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_cancelling_ignores_endpoint_not_found
```

Expected: PASS. The current resource-priority implementation should already satisfy both.

- [ ] **Step 4: Commit**

Run:

```bash
git add src-tauri/src/remote_workspace/service.rs
git commit -m "test(remote-workspace): cover idempotent cancellation cleanup"
```

---

### Task 8: Preserve Snapshots On Cancellation Cleanup Failure

**Files:**
- Modify: `src-tauri/src/remote_workspace/service.rs`

- [ ] **Step 1: Write endpoint cleanup failure test**

In `src-tauri/src/remote_workspace/service.rs`, add:

```rust
#[test]
fn provision_workspace_cancelling_endpoint_cleanup_failure_marks_failed_and_preserves_snapshots() {
    let state = Arc::new(Mutex::new(ProviderState {
        delete_endpoint_error: Some(RemoteWorkspaceError::Provider(
            ProviderError::RequestFailed {
                message: "provider request failed".to_string(),
            },
        )),
        ..ProviderState::default()
    }));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = workspace_with_all_remote_resources(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
        phase: Some(RemoteProvisioningPhase::CreatingRemoteEndpoint),
    };
    remote.remote_provisioning.percent = Some(75);

    let cancelled = block_on(service.provision_workspace(&workspace))
        .expect("cleanup failure should be represented in workspace state");

    let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
    assert_eq!(
        remote.remote_resources,
        RemoteWorkspaceResources {
            remote_volume: Some(RemoteVolumeSnapshot {
                id: "volume".to_string(),
            }),
            remote_provisioner: Some(RemoteProvisionerSnapshot {
                id: "provisioner".to_string(),
                status_url: "https://status.example".to_string(),
            }),
            remote_endpoint: Some(RemoteEndpointSnapshot {
                id: "endpoint".to_string(),
                url: "https://endpoint.example".to_string(),
            }),
        }
    );
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::Failed {
            phase: Some(RemoteProvisioningPhase::CreatingRemoteEndpoint),
            error: RemoteProvisioningError::Provider(ProviderError::RequestFailed {
                message: "provider request failed".to_string(),
            }),
        }
    );
    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec!["delete_endpoint"]
    );
}
```

- [ ] **Step 2: Write provisioner cleanup failure test**

In `src-tauri/src/remote_workspace/service.rs`, add:

```rust
#[test]
fn provision_workspace_cancelling_provisioner_cleanup_failure_marks_failed_and_preserves_snapshots() {
    let state = Arc::new(Mutex::new(ProviderState {
        terminate_provisioner_error: Some(RemoteWorkspaceError::Provider(
            ProviderError::RequestFailed {
                message: "provider request failed".to_string(),
            },
        )),
        ..ProviderState::default()
    }));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
        id: "volume".to_string(),
    });
    remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
        id: "provisioner".to_string(),
        status_url: "https://status.example".to_string(),
    });
    remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
        phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
            status: RemoteProvisionerStatus::Running,
        }),
    };
    remote.remote_provisioning.percent = Some(60);

    let cancelled = block_on(service.provision_workspace(&workspace))
        .expect("cleanup failure should be represented in workspace state");

    let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
    assert_eq!(
        remote.remote_resources.remote_provisioner,
        Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        })
    );
    assert_eq!(
        remote.remote_resources.remote_volume,
        Some(RemoteVolumeSnapshot {
            id: "volume".to_string(),
        })
    );
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::Failed {
            phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::Running,
            }),
            error: RemoteProvisioningError::Provider(ProviderError::RequestFailed {
                message: "provider request failed".to_string(),
            }),
        }
    );
    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec!["terminate_provisioner"]
    );
}
```

- [ ] **Step 3: Write volume cleanup failure test**

In `src-tauri/src/remote_workspace/service.rs`, add:

```rust
#[test]
fn provision_workspace_cancelling_volume_cleanup_failure_marks_failed_and_preserves_snapshot() {
    let state = Arc::new(Mutex::new(ProviderState {
        delete_volume_error: Some(RemoteWorkspaceError::Provider(
            ProviderError::RequestFailed {
                message: "provider request failed".to_string(),
            },
        )),
        ..ProviderState::default()
    }));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
        id: "volume".to_string(),
    });
    remote.remote_provisioning.status = RemoteProvisioningStatus::Cancelling {
        phase: Some(RemoteProvisioningPhase::StartingRemoteProvisioner),
    };
    remote.remote_provisioning.percent = Some(25);

    let cancelled = block_on(service.provision_workspace(&workspace))
        .expect("cleanup failure should be represented in workspace state");

    let WorkspaceRuntime::Remote(remote) = cancelled.runtime;
    assert_eq!(
        remote.remote_resources.remote_volume,
        Some(RemoteVolumeSnapshot {
            id: "volume".to_string(),
        })
    );
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::Failed {
            phase: Some(RemoteProvisioningPhase::StartingRemoteProvisioner),
            error: RemoteProvisioningError::Provider(ProviderError::RequestFailed {
                message: "provider request failed".to_string(),
            }),
        }
    );
    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec!["delete_volume"]
    );
}
```

- [ ] **Step 4: Run the focused tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_cancelling_endpoint_cleanup_failure_marks_failed_and_preserves_snapshots
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_cancelling_provisioner_cleanup_failure_marks_failed_and_preserves_snapshots
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_cancelling_volume_cleanup_failure_marks_failed_and_preserves_snapshot
```

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```bash
git add src-tauri/src/remote_workspace/service.rs
git commit -m "test(remote-workspace): preserve snapshots on cancellation failure"
```

---

### Task 9: Run Full Native Verification

**Files:**
- Verify: native Rust backend

- [ ] **Step 1: Run all native tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 2: Run Rust formatting check**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: PASS.

- [ ] **Step 3: Run Clippy**

Run:

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: PASS.

- [ ] **Step 4: Inspect generated TypeScript necessity**

Run:

```bash
git diff -- src/generated/commands.ts src-tauri/src/domain/workspace.rs src-tauri/src/lib.rs
```

Expected: `src-tauri/src/domain/workspace.rs` changed, but `src/generated/commands.ts` was not regenerated in this native-core plan.

If the implementation exports `RemoteProvisioningError` through command bindings in the current branch, run:

```bash
bun run codegen:commands
bun run build
bun run lint
```

Expected: PASS.

- [ ] **Step 5: Final commit**

Run:

```bash
git status --short
git add src-tauri/src/domain/workspace.rs src-tauri/src/remote_workspace/service.rs
git commit -m "feat(remote-workspace): cancel provisioning by rolling back resources"
```

Skip the commit if every prior task was already committed and `git status --short` is clean.

---

## Implementation Notes

- Cancellation is resource-driven, not phase-driven. The next cleanup action should be chosen by highest-order snapshot present: endpoint, then provisioner, then volume.
- The stored cancellation phase is still useful for UI context and failure reporting, but missing snapshots are treated as already cleaned up.
- The provisioner is terminated immediately during user cancellation. Do not poll `get_provisioner_status` in cancellation.
- Keep all errors UI-safe. Do not include provider keys, bearer tokens, worker payloads, logs, or filesystem dumps in failure state.
- Do not wire React or command persistence in this plan. The command layer can later call `cancel_workspace` to mark state, then use existing sync behavior to continue rollback.

## Self-Review

- Spec coverage: covers marking cancellation, one-action-per-sync cleanup, endpoint/provisioner/volume rollback, success reset, missing snapshots, not-found idempotency, cleanup failure preservation, and verification.
- Placeholder scan: no unresolved placeholders or vague test instructions.
- Type consistency: uses active repo types: `RemoteProvisioningStatus::Cancelling`, `RemoteProvisioningPhase::{CreatingRemoteEndpoint, RunningRemoteProvisioner, StartingRemoteProvisioner}`, provider cleanup params, and existing fake provider state.
