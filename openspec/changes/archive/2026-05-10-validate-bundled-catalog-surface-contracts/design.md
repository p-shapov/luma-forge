## Context

LumaForge reads bundled Workflow Catalog, Provisioning Profile, and Endpoint Profile JSON during Workspace Setup. The Native Layer exposes these catalogs to React and persists selected catalog objects into Draft Workspace records. Later Workspace Provisioning will use those persisted objects to create provider resources, start worker containers, clone Git repositories, download Hugging Face files, mount provider storage, and call worker HTTP endpoints.

Current validation rejects empty catalogs, duplicate profile/preset ids, zero sizes, and unsafe model asset install paths. It does not consistently validate adjacent fields that have the same future safety profile: Custom Node checkout paths, optional requirements paths, Git source fields, Hugging Face source paths, Docker image refs, mount paths, HTTP paths, or enum-like provider config values. The Docker image contract also includes `docker_image_digest`, but v1 does not use it as an execution or authenticity boundary.

## Goals / Non-Goals

**Goals:**

- Make bundled catalog validation deterministic, offline, and stricter about local contract shape.
- Reject unsafe or malformed catalog data before it is exposed to React or accepted during Workspace creation.
- Align Custom Node path semantics across native contracts, reference contracts, generated TypeScript bindings, and the Provisioner Worker schema.
- Remove misleading Docker digest metadata from the v1 contract.
- Preserve worker-side validation as the final safety boundary before remote filesystem writes.

**Non-Goals:**

- Verify that Docker images exist or that a digest belongs to an image.
- Verify Git repository reachability or revision existence.
- Verify Hugging Face repository or file existence.
- Call RunPod or any other Provider during bundled catalog validation.
- Add supply-chain image pinning in v1.

## Decisions

### Validate catalog surface shape, not external truth

The Native Layer SHALL validate local syntax and safety constraints for bundled catalog fields: required presence, non-empty strings, path shape, supported enum-like values, and port/range relationships. It SHALL NOT perform network calls or authenticity checks. This keeps catalog reads fast, deterministic, and suitable for startup/UI flows.

Alternative considered: make bundled validation verify Docker, Git, Hugging Face, or Provider resources. That would couple local catalog reads to network availability and turn operational failures into catalog-unavailable errors, which is not appropriate for Workspace Setup.

### Use direct Docker image refs

The v1 Docker image contract SHALL retain `docker_image_ref` directly on worker runtime objects and remove both `docker_image_digest` and the one-field Docker image wrapper. If provisioning submits only image refs and does not enforce digest-pinned execution, carrying a digest field creates misleading metadata. A one-field wrapper does not add a useful domain invariant once the digest is gone.

Alternative considered: keep a required or optional digest with format-only validation, or keep a one-field `DockerImage` object. The digest keeps future metadata but still suggests an integrity boundary that v1 does not provide; the wrapper adds shape without behavior.

### Make Custom Node requirements explicit optional data

`python_requirements_path` SHALL become optional in the native/reference/generated contracts. When absent, dependency installation for that Custom Node is skipped. When present, it SHALL be a safe relative path resolved under the Custom Node checkout root.

Alternative considered: keep an empty string sentinel. That makes absence look like invalid path data and forces every consumer to preserve a special case.

### Keep distinct path roots

The catalog uses different path roots:

- Model asset `comfyui_relative_path`: relative to the prepared ComfyUI root.
- Custom Node `comfyui_custom_nodes_relative_path`: relative to the prepared ComfyUI root and constrained under `custom_nodes/...`.
- Custom Node `python_requirements_path`: relative to the Custom Node checkout root.
- Provider/container mount paths: absolute normalized POSIX paths and not `/`.
- HTTP paths: absolute HTTP path components starting with `/`, with no query or fragment.

This avoids one generic "relative path" rule hiding different filesystem meanings.

### Validate URL-shaped source fields without reachability checks

Git repository URLs SHALL be URL-shaped. Hugging Face repository ids SHALL be repo-id shaped, model source file paths SHALL be safe repo-relative paths with nested segments allowed, and revisions SHALL be non-empty. The validator SHALL NOT check that those external resources exist.

## Risks / Trade-offs

- Existing catalog data may fail stricter validation if placeholders or loosely shaped values remain. Mitigation: update bundled JSON in the same implementation and add negative tests for each rejected shape.
- Removing `docker_image_digest` changes generated frontend bindings and reference contracts. Mitigation: regenerate bindings and update all call sites in the same change.
- URL-shaped validation can become either too permissive or too restrictive. Mitigation: keep validation intentionally shallow and focused on supported v1 catalog sources.
- Optional requirements paths require Rust, TypeScript, and Python schema alignment. Mitigation: add tests for absent, null, blank, safe, and unsafe requirements values where each runtime parses catalog/request data.

## Migration Plan

1. Update reference contracts and Rust DTO/domain types to remove `docker_image_digest`, remove the Docker image wrapper, and make `python_requirements_path` optional.
2. Update bundled JSON catalogs to match the new contracts.
3. Add shared validator helpers for safe relative paths, custom-node paths, absolute POSIX mount paths, HTTP paths, URL-shaped strings, repo ids, and enum-like strings.
4. Extend bundled catalog validation and tests.
5. Align Provisioner Worker request parsing/tests with the optional requirements contract.
6. Regenerate frontend command bindings.

Rollback before release can restore the previous catalog contract and bundled JSON shape because no released provisioning flow depends on the removed digest field yet.

## Open Questions

None.
