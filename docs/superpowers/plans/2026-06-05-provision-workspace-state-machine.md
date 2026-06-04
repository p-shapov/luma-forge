# Provision Workspace State Machine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete `RemoteWorkspaceService::provision_workspace` as a legacy-style one-action-per-call sync state machine and remove provider resource observation/conflict behavior.

**Architecture:** Keep the active `src-tauri/src/remote_workspace` skeleton in place. The service remains the workflow owner, provider traits remain resource primitives, and tests drive each state transition before implementation. The active domain gets the minimum enum shape needed to carry terminal worker status through cleanup and represent legacy provisioner-worker failure cases.

**Tech Stack:** Rust, Tauri native backend, unit tests in `src-tauri`, existing `AppFuture` boxed-future trait pattern.

---

## File Structure

- Modify `src-tauri/src/domain/workspace.rs`
  - Add terminal worker status payload to `RemoteProvisioningPhase::CleaningUpRemoteProvisioner`.
  - Keep `RemoteProvisionerStatus` as the source of worker terminal success/failure data.
- Modify `src-tauri/src/remote_workspace/errors.rs`
  - Remove existing-resource conflict variants.
  - Add provisioner-worker error variants from legacy workspace failure codes.
  - Add a cleanup failure variant only if needed for UI-safe cleanup failure state.
- Modify `src-tauri/src/remote_workspace/provider.rs`
  - Delete `ObserveVolumeParams`, `ObserveProvisionerParams`, `ObserveEndpointParams`.
  - Delete `observe_volume`, `observe_provisioner`, and `observe_endpoint`.
  - Keep create/start/status/terminate/create-endpoint/delete primitives.
- Modify `src-tauri/src/remote_workspace/registry.rs`
  - Remove observe method implementations from registry tests' fake provider.
- Modify `src-tauri/src/remote_workspace/service.rs`
  - Delete `observe_workspace`.
  - Update fake provider state and tests.
  - Implement full one-step provisioning state machine.

---

### Task 1: Remove Observe Boundary And Existing-Resource Errors

**Files:**
- Modify: `src-tauri/src/remote_workspace/provider.rs`
- Modify: `src-tauri/src/remote_workspace/errors.rs`
- Modify: `src-tauri/src/remote_workspace/registry.rs`
- Modify: `src-tauri/src/remote_workspace/service.rs`

- [ ] **Step 1: Delete observe params and trait methods**

In `src-tauri/src/remote_workspace/provider.rs`, remove these structs:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserveVolumeParams {
    pub workspace_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserveProvisionerParams {
    pub workspace_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserveEndpointParams {
    pub workspace_id: String,
    pub endpoint_id: Option<String>,
}
```

Remove these trait methods:

```rust
fn observe_volume<'a>(
    &'a self,
    params: ObserveVolumeParams,
) -> AppFuture<'a, Result<Option<RemoteVolumeSnapshot>, RemoteWorkspaceError>>;

fn observe_provisioner<'a>(
    &'a self,
    params: ObserveProvisionerParams,
) -> AppFuture<'a, Result<Option<RemoteProvisionerSnapshot>, RemoteWorkspaceError>>;

fn observe_endpoint<'a>(
    &'a self,
    params: ObserveEndpointParams,
) -> AppFuture<'a, Result<Option<RemoteEndpointSnapshot>, RemoteWorkspaceError>>;
```

- [ ] **Step 2: Remove existing-resource error variants**

In `src-tauri/src/remote_workspace/errors.rs`, remove:

```rust
ExistingVolume,
ExistingProvisioner,
ExistingEndpoint,
```

Keep the `NonExisting*` variants used by `delete_workspace`.

- [ ] **Step 3: Remove `observe_workspace` from the service**

In `src-tauri/src/remote_workspace/service.rs`, delete the full `observe_workspace` method and remove observe imports from the `provider::{ ... }` import list.

Delete these tests:

```rust
observe_workspace_returns_existing_volume_conflict
observe_workspace_returns_existing_provisioner_conflict
observe_workspace_returns_existing_endpoint_conflict
observe_workspace_returns_provider_request_failure
```

- [ ] **Step 4: Update fake providers to compile without observe methods**

In `src-tauri/src/remote_workspace/registry.rs` and `src-tauri/src/remote_workspace/service.rs`, remove fake provider implementations of `observe_volume`, `observe_provisioner`, and `observe_endpoint`.

Remove these `ProviderState` fields from `service.rs` tests:

```rust
volume: Option<RemoteVolumeSnapshot>,
provisioner: Option<RemoteProvisionerSnapshot>,
endpoint: Option<RemoteEndpointSnapshot>,
observe_volume_error: Option<RemoteWorkspaceError>,
observe_provisioner_error: Option<RemoteWorkspaceError>,
observe_endpoint_error: Option<RemoteWorkspaceError>,
```

- [ ] **Step 5: Update the `NotStarted` test expectation**

Rename `provision_workspace_not_started_runs_preflight_then_creates_volume_only` to:

```rust
fn provision_workspace_not_started_creates_volume_only()
```

Update the expected provider calls:

```rust
assert_eq!(
    state.lock().expect("state lock should succeed").calls,
    vec!["create_volume"]
);
```

- [ ] **Step 6: Run focused compile check**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-run
```

Expected: the test target compiles. If this fails with references to observe params or existing-resource variants, remove the remaining references.

- [ ] **Step 7: Commit**

Run:

```bash
git add src-tauri/src/remote_workspace/provider.rs src-tauri/src/remote_workspace/errors.rs src-tauri/src/remote_workspace/registry.rs src-tauri/src/remote_workspace/service.rs
git commit -m "refactor(remote-workspace): remove resource observation boundary"
```

---

### Task 2: Add Worker Failure Error Cases And Cleanup Phase Payload

**Files:**
- Modify: `src-tauri/src/domain/workspace.rs`
- Modify: `src-tauri/src/remote_workspace/errors.rs`
- Modify: `src-tauri/src/remote_workspace/service.rs`
- Modify: `src-tauri/src/remote_workspace/registry.rs`

- [ ] **Step 1: Write failing enum shape test**

In `src-tauri/src/remote_workspace/service.rs`, add a test near the provisioning tests:

```rust
#[test]
fn cleaning_up_phase_preserves_terminal_worker_status() {
    let phase = RemoteProvisioningPhase::CleaningUpRemoteProvisioner {
        terminal_status: RemoteProvisionerStatus::Failed {
            code: "provisioner_worker_failed".to_string(),
            message: "worker failed".to_string(),
        },
    };

    assert_eq!(
        phase,
        RemoteProvisioningPhase::CleaningUpRemoteProvisioner {
            terminal_status: RemoteProvisionerStatus::Failed {
                code: "provisioner_worker_failed".to_string(),
                message: "worker failed".to_string(),
            },
        }
    );
}
```

- [ ] **Step 2: Run the failing enum shape test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml cleaning_up_phase_preserves_terminal_worker_status
```

Expected: FAIL because `CleaningUpRemoteProvisioner` does not yet accept `terminal_status`.

- [ ] **Step 3: Update `RemoteProvisioningPhase`**

In `src-tauri/src/domain/workspace.rs`, change:

```rust
CleaningUpRemoteProvisioner,
```

to:

```rust
CleaningUpRemoteProvisioner {
    terminal_status: RemoteProvisionerStatus,
},
```

Update any existing test setup using `RemoteProvisioningPhase::CleaningUpRemoteProvisioner` to provide:

```rust
RemoteProvisioningPhase::CleaningUpRemoteProvisioner {
    terminal_status: RemoteProvisionerStatus::Succeeded,
}
```

- [ ] **Step 4: Add worker error variants**

In `src-tauri/src/remote_workspace/errors.rs`, derive `Serialize` and configure snake-case serialization:

```rust
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteWorkspaceError {
```

Add:

```rust
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
```

- [ ] **Step 5: Run focused test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml cleaning_up_phase_preserves_terminal_worker_status
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
git add src-tauri/src/domain/workspace.rs src-tauri/src/remote_workspace/errors.rs src-tauri/src/remote_workspace/service.rs src-tauri/src/remote_workspace/registry.rs
git commit -m "feat(remote-workspace): add worker failure state cases"
```

---

### Task 3: Implement Worker Status Polling Transitions

**Files:**
- Modify: `src-tauri/src/remote_workspace/service.rs`

- [ ] **Step 1: Extend fake provider status support**

In the test `ProviderState`, add:

```rust
provisioner_status_results: Vec<Result<RemoteProvisionerStatus, RemoteWorkspaceError>>,
last_get_provisioner_status_params: Option<GetProvisionerStatusParams>,
```

Update fake `get_provisioner_status`:

```rust
fn get_provisioner_status<'a>(
    &'a self,
    params: GetProvisionerStatusParams,
) -> AppFuture<'a, Result<RemoteProvisionerStatus, RemoteWorkspaceError>> {
    Box::pin(async move {
        let mut state = self.state.lock().expect("state lock should succeed");
        state.calls.push("get_provisioner_status");
        state.last_get_provisioner_status_params = Some(params);
        if state.provisioner_status_results.is_empty() {
            return Ok(RemoteProvisionerStatus::Pending);
        }
        state.provisioner_status_results.remove(0)
    })
}
```

- [ ] **Step 2: Write non-terminal polling test**

Add:

```rust
#[test]
fn provision_workspace_running_provisioner_stores_non_terminal_status() {
    let state = Arc::new(Mutex::new(ProviderState {
        provisioner_status_results: vec![Ok(RemoteProvisionerStatus::Running)],
        ..ProviderState::default()
    }));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
        id: "provisioner".to_string(),
        status_url: "https://status.example".to_string(),
    });
    remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
        phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
            status: RemoteProvisionerStatus::Pending,
        },
    };
    remote.remote_provisioning.percent = Some(50);

    let provisioned = block_on(service.provision_workspace(&workspace))
        .expect("running provisioner should poll status");

    let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::Running
            }
        }
    );
    assert_eq!(remote.remote_provisioning.percent, Some(60));
    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec!["get_provisioner_status"]
    );
}
```

- [ ] **Step 3: Write terminal success transition test**

Add:

```rust
#[test]
fn provision_workspace_worker_success_moves_to_cleanup() {
    let state = Arc::new(Mutex::new(ProviderState {
        provisioner_status_results: vec![Ok(RemoteProvisionerStatus::Succeeded)],
        ..ProviderState::default()
    }));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
        id: "provisioner".to_string(),
        status_url: "https://status.example".to_string(),
    });
    remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
        phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
            status: RemoteProvisionerStatus::Running,
        },
    };

    let provisioned = block_on(service.provision_workspace(&workspace))
        .expect("worker success should move to cleanup");

    let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::CleaningUpRemoteProvisioner {
                terminal_status: RemoteProvisionerStatus::Succeeded
            }
        }
    );
    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec!["get_provisioner_status"]
    );
}
```

- [ ] **Step 4: Write terminal failure transition test**

Add:

```rust
#[test]
fn provision_workspace_worker_failure_moves_to_cleanup_with_failure_details() {
    let failed_status = RemoteProvisionerStatus::Failed {
        code: "provisioner_worker_asset_download_failed".to_string(),
        message: "asset download failed".to_string(),
    };
    let state = Arc::new(Mutex::new(ProviderState {
        provisioner_status_results: vec![Ok(failed_status.clone())],
        ..ProviderState::default()
    }));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
        id: "provisioner".to_string(),
        status_url: "https://status.example".to_string(),
    });
    remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
        phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
            status: RemoteProvisionerStatus::Running,
        },
    };

    let provisioned = block_on(service.provision_workspace(&workspace))
        .expect("worker failure should move to cleanup before failed state");

    let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::CleaningUpRemoteProvisioner {
                terminal_status: failed_status
            }
        }
    );
    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec!["get_provisioner_status"]
    );
}
```

- [ ] **Step 5: Implement running provisioner branch**

In `provision_workspace`, replace the unsupported phase fallback for `RunningRemoteProvisioner` with:

```rust
RemoteProvisioningStatus::InProgress {
    phase: RemoteProvisioningPhase::RunningRemoteProvisioner { .. },
} => {
    let remote_provisioner = remote.remote_resources.remote_provisioner.as_ref().ok_or(
        RemoteWorkspaceError::InvalidWorkspaceState {
            message: "remote provisioner snapshot is required before status polling"
                .to_string(),
        },
    )?;
    let provider_id = remote.remote_placement.gpu_cloud_provider_id;
    let provider = self.provider_registry.for_provider(provider_id)?;
    let status = provider
        .get_provisioner_status(GetProvisionerStatusParams {
            workspace_id: workspace.id.clone(),
            provisioner_id: remote_provisioner.id.clone(),
        })
        .await?;

    let mut workspace = workspace.clone();
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_provisioning.percent = Some(match status {
        RemoteProvisionerStatus::Pending | RemoteProvisionerStatus::Starting => 50,
        RemoteProvisionerStatus::Running => 60,
        RemoteProvisionerStatus::Succeeded | RemoteProvisionerStatus::Failed { .. } => 75,
        RemoteProvisionerStatus::Terminated => 75,
    });
    remote.remote_provisioning.status = match status {
        RemoteProvisionerStatus::Succeeded | RemoteProvisionerStatus::Failed { .. } => {
            RemoteProvisioningStatus::InProgress {
                phase: RemoteProvisioningPhase::CleaningUpRemoteProvisioner {
                    terminal_status: status,
                },
            }
        }
        status => RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::RunningRemoteProvisioner { status },
        },
    };

    Ok(workspace)
}
```

- [ ] **Step 6: Run focused polling tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_running_provisioner_stores_non_terminal_status
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_worker_success_moves_to_cleanup
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_worker_failure_moves_to_cleanup_with_failure_details
```

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```bash
git add src-tauri/src/remote_workspace/service.rs
git commit -m "feat(remote-workspace): poll provisioner status"
```

---

### Task 4: Implement Provisioner Cleanup Transitions

**Files:**
- Modify: `src-tauri/src/remote_workspace/service.rs`

- [ ] **Step 1: Write cleanup after success test**

Add:

```rust
#[test]
fn provision_workspace_cleanup_after_success_moves_to_endpoint_creation() {
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
        id: "provisioner".to_string(),
        status_url: "https://status.example".to_string(),
    });
    remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
        phase: RemoteProvisioningPhase::CleaningUpRemoteProvisioner {
            terminal_status: RemoteProvisionerStatus::Succeeded,
        },
    };

    let provisioned = block_on(service.provision_workspace(&workspace))
        .expect("cleanup after success should terminate provisioner");

    let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
    assert_eq!(remote.remote_resources.remote_provisioner, None);
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::InProgress {
            phase: RemoteProvisioningPhase::CreatingRemoteEndpoint
        }
    );
    assert_eq!(remote.remote_provisioning.percent, Some(75));
    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec!["terminate_provisioner"]
    );
}
```

- [ ] **Step 2: Write cleanup after worker failure test**

Add:

```rust
#[test]
fn provision_workspace_cleanup_after_worker_failure_marks_failed() {
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
    remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
        phase: RemoteProvisioningPhase::CleaningUpRemoteProvisioner {
            terminal_status: RemoteProvisionerStatus::Failed {
                code: "provisioner_worker_step_timeout".to_string(),
                message: "step timed out".to_string(),
            },
        },
    };

    let provisioned = block_on(service.provision_workspace(&workspace))
        .expect("cleanup after worker failure should mark failed");

    let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
    assert_eq!(remote.remote_resources.remote_provisioner, None);
    assert_eq!(remote.remote_resources.remote_volume, Some(RemoteVolumeSnapshot {
        id: "volume".to_string()
    }));
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::Failed {
            phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: RemoteProvisionerStatus::Failed {
                    code: "provisioner_worker_step_timeout".to_string(),
                    message: "step timed out".to_string(),
                },
            }),
            code: "provisioner_worker_step_timeout".to_string(),
            message: "step timed out".to_string(),
        }
    );
}
```

- [ ] **Step 3: Write termination failure after success test**

Add:

```rust
#[test]
fn provision_workspace_cleanup_error_after_success_marks_failed_and_preserves_provisioner() {
    let state = Arc::new(Mutex::new(ProviderState {
        terminate_provisioner_error: Some(RemoteWorkspaceError::ProviderRequestFailed {
            message: "terminate failed".to_string(),
        }),
        ..ProviderState::default()
    }));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
        id: "provisioner".to_string(),
        status_url: "https://status.example".to_string(),
    });
    remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
        phase: RemoteProvisioningPhase::CleaningUpRemoteProvisioner {
            terminal_status: RemoteProvisionerStatus::Succeeded,
        },
    };

    let provisioned = block_on(service.provision_workspace(&workspace))
        .expect("cleanup error after success should become failed workspace");

    let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
    assert_eq!(
        remote.remote_resources.remote_provisioner,
        Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        })
    );
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::Failed {
            phase: Some(RemoteProvisioningPhase::CleaningUpRemoteProvisioner {
                terminal_status: RemoteProvisionerStatus::Succeeded,
            }),
            code: "cleanup_failed".to_string(),
            message: "terminate failed".to_string(),
        }
    );
}
```

- [ ] **Step 4: Write termination failure after worker failure test**

Add:

```rust
#[test]
fn provision_workspace_cleanup_error_after_worker_failure_preserves_worker_failure() {
    let terminal_status = RemoteProvisionerStatus::Failed {
        code: "provisioner_worker_unexpected_error".to_string(),
        message: "unexpected worker error".to_string(),
    };
    let state = Arc::new(Mutex::new(ProviderState {
        terminate_provisioner_error: Some(RemoteWorkspaceError::ProviderRequestFailed {
            message: "terminate failed".to_string(),
        }),
        ..ProviderState::default()
    }));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_resources.remote_provisioner = Some(RemoteProvisionerSnapshot {
        id: "provisioner".to_string(),
        status_url: "https://status.example".to_string(),
    });
    remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
        phase: RemoteProvisioningPhase::CleaningUpRemoteProvisioner {
            terminal_status: terminal_status.clone(),
        },
    };

    let provisioned = block_on(service.provision_workspace(&workspace))
        .expect("cleanup error after worker failure should preserve worker failure");

    let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
    assert_eq!(
        remote.remote_resources.remote_provisioner,
        Some(RemoteProvisionerSnapshot {
            id: "provisioner".to_string(),
            status_url: "https://status.example".to_string(),
        })
    );
    assert_eq!(
        remote.remote_provisioning.status,
        RemoteProvisioningStatus::Failed {
            phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                status: terminal_status,
            }),
            code: "provisioner_worker_unexpected_error".to_string(),
            message: "unexpected worker error".to_string(),
        }
    );
}
```

- [ ] **Step 5: Add cleanup helper**

In `src-tauri/src/remote_workspace/service.rs`, add:

```rust
fn provider_error_message(error: RemoteWorkspaceError) -> String {
    match error {
        RemoteWorkspaceError::ProviderRequestFailed { message } => message,
        error => format!("{error:?}"),
    }
}
```

- [ ] **Step 6: Implement cleanup branch**

In `provision_workspace`, add a branch for:

```rust
RemoteProvisioningStatus::InProgress {
    phase:
        RemoteProvisioningPhase::CleaningUpRemoteProvisioner {
            terminal_status,
        },
} => {
    let remote_provisioner = remote.remote_resources.remote_provisioner.as_ref().ok_or(
        RemoteWorkspaceError::InvalidWorkspaceState {
            message: "remote provisioner snapshot is required before provisioner cleanup"
                .to_string(),
        },
    )?;
    let provider_id = remote.remote_placement.gpu_cloud_provider_id;
    let provider = self.provider_registry.for_provider(provider_id)?;
    let termination_result = provider
        .terminate_provisioner(TerminateProvisionerParams {
            workspace_id: workspace.id.clone(),
            provisioner_id: remote_provisioner.id.clone(),
        })
        .await;

    let mut workspace = workspace.clone();
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;

    match (terminal_status.clone(), termination_result) {
        (RemoteProvisionerStatus::Succeeded, Ok(())) => {
            remote.remote_resources.remote_provisioner = None;
            remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
                phase: RemoteProvisioningPhase::CreatingRemoteEndpoint,
            };
            remote.remote_provisioning.percent = Some(75);
        }
        (RemoteProvisionerStatus::Succeeded, Err(error)) => {
            remote.remote_provisioning.status = RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::CleaningUpRemoteProvisioner {
                    terminal_status: RemoteProvisionerStatus::Succeeded,
                }),
                code: "cleanup_failed".to_string(),
                message: provider_error_message(error),
            };
        }
        (RemoteProvisionerStatus::Failed { code, message }, Ok(())) => {
            remote.remote_resources.remote_provisioner = None;
            remote.remote_provisioning.status = RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::Failed {
                        code: code.clone(),
                        message: message.clone(),
                    },
                }),
                code,
                message,
            };
        }
        (RemoteProvisionerStatus::Failed { code, message }, Err(_)) => {
            remote.remote_provisioning.status = RemoteProvisioningStatus::Failed {
                phase: Some(RemoteProvisioningPhase::RunningRemoteProvisioner {
                    status: RemoteProvisionerStatus::Failed {
                        code: code.clone(),
                        message: message.clone(),
                    },
                }),
                code,
                message,
            };
        }
        (status, _) => {
            return Err(RemoteWorkspaceError::InvalidWorkspaceState {
                message: format!("cleanup requires terminal provisioner status: {status:?}"),
            });
        }
    }

    Ok(workspace)
}
```

- [ ] **Step 7: Run focused cleanup tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_cleanup_after_success_moves_to_endpoint_creation
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_cleanup_after_worker_failure_marks_failed
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_cleanup_error_after_success_marks_failed_and_preserves_provisioner
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_cleanup_error_after_worker_failure_preserves_worker_failure
```

Expected: PASS.

- [ ] **Step 8: Commit**

Run:

```bash
git add src-tauri/src/remote_workspace/service.rs
git commit -m "feat(remote-workspace): clean up provisioner after terminal worker status"
```

---

### Task 5: Implement Endpoint Creation Completion

**Files:**
- Modify: `src-tauri/src/remote_workspace/service.rs`

- [ ] **Step 1: Extend fake provider endpoint tracking**

In `ProviderState`, add:

```rust
create_endpoint_error: Option<RemoteWorkspaceError>,
last_create_endpoint_params: Option<CreateEndpointParams>,
```

Update fake `create_endpoint`:

```rust
fn create_endpoint<'a>(
    &'a self,
    params: CreateEndpointParams,
) -> AppFuture<'a, Result<RemoteEndpointSnapshot, RemoteWorkspaceError>> {
    Box::pin(async move {
        let mut state = self.state.lock().expect("state lock should succeed");
        state.calls.push("create_endpoint");
        state.last_create_endpoint_params = Some(params);
        if let Some(error) = state.create_endpoint_error.clone() {
            return Err(error);
        }
        Ok(RemoteEndpointSnapshot {
            id: "endpoint".to_string(),
            url: "https://endpoint.example".to_string(),
        })
    })
}
```

- [ ] **Step 2: Write endpoint completion test**

Add:

```rust
#[test]
fn provision_workspace_creating_endpoint_marks_completed() {
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_resources.remote_volume = Some(RemoteVolumeSnapshot {
        id: "volume".to_string(),
    });
    remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
        phase: RemoteProvisioningPhase::CreatingRemoteEndpoint,
    };

    let provisioned = block_on(service.provision_workspace(&workspace))
        .expect("endpoint creation should complete workspace");

    let WorkspaceRuntime::Remote(remote) = provisioned.runtime;
    assert_eq!(
        remote.remote_resources.remote_endpoint,
        Some(RemoteEndpointSnapshot {
            id: "endpoint".to_string(),
            url: "https://endpoint.example".to_string(),
        })
    );
    assert_eq!(remote.remote_provisioning.status, RemoteProvisioningStatus::Completed);
    assert_eq!(remote.remote_provisioning.percent, Some(100));
    assert_eq!(
        state.lock().expect("state lock should succeed").calls,
        vec!["create_endpoint"]
    );
}
```

- [ ] **Step 3: Write missing volume test**

Add:

```rust
#[test]
fn provision_workspace_creating_endpoint_without_volume_returns_invalid_state() {
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
        phase: RemoteProvisioningPhase::CreatingRemoteEndpoint,
    };

    let error = block_on(service.provision_workspace(&workspace))
        .expect_err("missing volume should stop endpoint creation");

    assert_eq!(
        error,
        RemoteWorkspaceError::InvalidWorkspaceState {
            message: "remote volume snapshot is required before endpoint creation".to_string(),
        }
    );
    assert!(state.lock().expect("state lock should succeed").calls.is_empty());
}
```

- [ ] **Step 4: Add unresolved endpoint image constant**

Near `UNRESOLVED_PROVISIONER_IMAGE_REF`, add:

```rust
const UNRESOLVED_ENDPOINT_IMAGE_REF: &str = "unresolved-endpoint-image";
```

- [ ] **Step 5: Implement endpoint branch**

In `provision_workspace`, add:

```rust
RemoteProvisioningStatus::InProgress {
    phase: RemoteProvisioningPhase::CreatingRemoteEndpoint,
} => {
    let remote_volume = remote.remote_resources.remote_volume.as_ref().ok_or(
        RemoteWorkspaceError::InvalidWorkspaceState {
            message: "remote volume snapshot is required before endpoint creation".to_string(),
        },
    )?;
    let provider_id = remote.remote_placement.gpu_cloud_provider_id;
    let provider = self.provider_registry.for_provider(provider_id)?;
    let remote_endpoint = provider
        .create_endpoint(CreateEndpointParams {
            workspace_id: workspace.id.clone(),
            datacenter_id: remote.remote_placement.datacenter_id.clone(),
            gpu_id: remote.remote_placement.gpu_id.clone(),
            volume_id: remote_volume.id.clone(),
            endpoint_image_ref: UNRESOLVED_ENDPOINT_IMAGE_REF.to_string(),
            mount_path: "/workspace".to_string(),
        })
        .await?;

    let mut workspace = workspace.clone();
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_resources.remote_endpoint = Some(remote_endpoint);
    remote.remote_provisioning.status = RemoteProvisioningStatus::Completed;
    remote.remote_provisioning.percent = Some(100);

    Ok(workspace)
}
```

- [ ] **Step 6: Run focused endpoint tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_creating_endpoint_marks_completed
cargo test --manifest-path src-tauri/Cargo.toml provision_workspace_creating_endpoint_without_volume_returns_invalid_state
```

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```bash
git add src-tauri/src/remote_workspace/service.rs
git commit -m "feat(remote-workspace): complete endpoint provisioning"
```

---

### Task 6: Update Remaining Unsupported Phase And Regression Tests

**Files:**
- Modify: `src-tauri/src/remote_workspace/service.rs`

- [ ] **Step 1: Replace obsolete unsupported phase test**

Delete:

```rust
provision_workspace_unsupported_in_progress_phase_returns_not_implemented_without_provider_calls
```

Add:

```rust
#[test]
fn provision_workspace_validating_readiness_returns_not_implemented_without_provider_calls() {
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
        phase: RemoteProvisioningPhase::ValidatingReadiness,
    };

    let error = block_on(service.provision_workspace(&workspace))
        .expect_err("readiness validation remains unimplemented");

    assert_eq!(
        error,
        RemoteWorkspaceError::NotImplemented {
            message: "readiness validation is not implemented in this skeleton".to_string(),
        }
    );
    assert!(state.lock().expect("state lock should succeed").calls.is_empty());
}
```

- [ ] **Step 2: Add missing provisioner status test**

Add:

```rust
#[test]
fn provision_workspace_running_provisioner_without_snapshot_returns_invalid_state() {
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
        phase: RemoteProvisioningPhase::RunningRemoteProvisioner {
            status: RemoteProvisionerStatus::Running,
        },
    };

    let error = block_on(service.provision_workspace(&workspace))
        .expect_err("missing provisioner should stop polling");

    assert_eq!(
        error,
        RemoteWorkspaceError::InvalidWorkspaceState {
            message: "remote provisioner snapshot is required before status polling".to_string(),
        }
    );
    assert!(state.lock().expect("state lock should succeed").calls.is_empty());
}
```

- [ ] **Step 3: Add missing cleanup provisioner test**

Add:

```rust
#[test]
fn provision_workspace_cleanup_without_provisioner_returns_invalid_state() {
    let state = Arc::new(Mutex::new(ProviderState::default()));
    let service = service_with_state(Arc::clone(&state));
    let mut workspace = draft_workspace(&service);
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_provisioning.status = RemoteProvisioningStatus::InProgress {
        phase: RemoteProvisioningPhase::CleaningUpRemoteProvisioner {
            terminal_status: RemoteProvisionerStatus::Succeeded,
        },
    };

    let error = block_on(service.provision_workspace(&workspace))
        .expect_err("missing provisioner should stop cleanup");

    assert_eq!(
        error,
        RemoteWorkspaceError::InvalidWorkspaceState {
            message: "remote provisioner snapshot is required before provisioner cleanup"
                .to_string(),
        }
    );
    assert!(state.lock().expect("state lock should succeed").calls.is_empty());
}
```

- [ ] **Step 4: Update final unsupported fallback**

Change the remaining unsupported fallback to:

```rust
RemoteProvisioningStatus::InProgress {
    phase: RemoteProvisioningPhase::ValidatingReadiness,
} => Err(RemoteWorkspaceError::NotImplemented {
    message: "readiness validation is not implemented in this skeleton".to_string(),
}),
RemoteProvisioningStatus::Cancelling { .. } => Err(RemoteWorkspaceError::NotImplemented {
    message: "provisioning cancellation is not implemented in this skeleton".to_string(),
}),
```

- [ ] **Step 5: Run all remote workspace tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml remote_workspace
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
git add src-tauri/src/remote_workspace/service.rs
git commit -m "test(remote-workspace): cover invalid provisioning phases"
```

---

### Task 7: Full Verification

**Files:**
- Modify only files changed in previous tasks if verification finds compile, format, or lint failures.

- [ ] **Step 1: Run native backend tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 2: Run formatting check**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: PASS.

If it fails, run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
```

Then rerun the check.

- [ ] **Step 3: Run clippy**

Run:

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: PASS.

- [ ] **Step 4: Run frontend contract verification only if generated bindings changed**

If `src/generated/commands.ts` changes or `cargo test` reports Specta/export mismatches, run:

```bash
bun run codegen:commands
bun run build
bun run lint
```

Expected: all commands PASS.

- [ ] **Step 5: Commit verification fixes**

If verification changed files, run:

```bash
git add src-tauri src/generated/commands.ts
git commit -m "fix(remote-workspace): satisfy provisioning verification"
```

If verification did not change files, do not create an empty commit.
