## 1. RunPod Pod Response Mapping

- [x] 1.1 Update RunPod pod observation mapping so create calls can use request-derived selected data center and selected GPU when the response omits those fields.
- [x] 1.2 Derive Provisioner Worker status URLs for `<port>/http` pod exposure as `https://<pod-id>-<port>.proxy.runpod.net/status`.
- [x] 1.3 Keep direct `publicIp` plus `portMappings` support only as the direct TCP fallback path.
- [x] 1.4 Add RunPod mapper tests for HTTP pod responses with empty `publicIp`, missing `portMappings`, missing data center, and missing GPU fields.

## 2. Provider Discovery and Adoption

- [x] 2.1 Add a provider gateway operation to discover live RunPod provisioning pods correlated by Workspace-derived pod name and network volume id.
- [x] 2.2 Implement RunPod list/filter support without exposing pod environment values, provider payloads, API keys, or worker tokens outside the provider boundary.
- [x] 2.3 Add registry tests for zero, one, and multiple matching provisioning pod discovery results.
- [x] 2.4 Ensure discovery ignores terminated/deleted pods and does not adopt resources without the expected Workspace volume correlation.

## 3. Workspace Provisioning Service

- [x] 3.1 Before creating a provisioning pod, call provider discovery when the Workspace has a ready volume and no active pod snapshot.
- [x] 3.2 Adopt exactly one matching live pod by persisting an active Provisioning Pod snapshot and returning progress without creating a new pod.
- [x] 3.3 Mark the Workspace `failed` and avoid provider mutation when multiple matching live pods are discovered.
- [x] 3.4 Persist an active Provisioning Pod snapshot immediately after a create response contains a provider pod id, even when RunPod omits direct TCP metadata.
- [x] 3.5 Preserve existing Provisioner Worker status URL during later pod observations that omit connectivity metadata.
- [x] 3.6 Add service tests proving repeated sync after an incomplete create response does not create a second pod.
- [x] 3.7 Add service tests proving current orphan-like state adopts one matching pod and fails closed for multiple matching pods.

## 4. Failure and Cleanup Metadata

- [x] 4.1 Extend provisioning failure construction as needed so duplicate correlated pods surface cleanup/inspection recovery without losing known volume metadata.
- [x] 4.2 Ensure command error mapping and progress output remain UI-safe and do not expose provider payloads or secrets.
- [x] 4.3 Add tests for the duplicate-pod failure path and retained cleanup metadata.

## 5. Verification

- [x] 5.1 Run `cargo test`.
- [x] 5.2 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 5.3 Run `cargo fmt`.
- [x] 5.4 Run `bun run build`.
- [x] 5.5 Run `bun run lint --fix`.
- [x] 5.6 Run `openspec status --change fix-runpod-provisioning-pod-idempotency` and confirm the change is apply-ready.
