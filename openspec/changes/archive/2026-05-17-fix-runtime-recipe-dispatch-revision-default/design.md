## Context

Runtime recipe releases publish a provisioner/endpoint image pair and then open a reviewed Runtime Catalog update. The workflow accepts `implementation_revision` during manual dispatch, but the current default is `2026.05.16-001`, which is already present in `bundled/runtime-catalog.json`. Because catalog compatibility validation rejects duplicate implementation revisions before build and publish, a default manual dispatch fails immediately.

Implementation revisions are part of the immutable runtime implementation identity. They need to be deliberate release identifiers, and the catalog duplicate check must remain authoritative.

## Goals / Non-Goals

**Goals:**

- Make the default manual runtime recipe dispatch path usable.
- Prevent operators from accidentally dispatching a release with a known duplicate implementation revision.
- Fail before worker validation, Docker builds, registry publication, or catalog PR creation when a duplicate revision is selected.
- Keep implementation revision immutability enforced by the existing release tooling and Runtime Catalog validation.

**Non-Goals:**

- Change the Runtime Catalog schema.
- Change runtime contract compatibility rules.
- Change tag-triggered release semantics except where they share validation with manual dispatch.
- Automatically mutate or replace existing Runtime Catalog implementation revisions.

## Decisions

1. Remove the stale `workflow_dispatch` default rather than replacing it with another fixed revision.

   A fixed default will eventually become stale after the next successful release. Requiring the operator to provide a release revision keeps the workflow honest and matches the existing deployment documentation, which already describes manual runs as selecting a recipe plus an implementation revision.

2. Keep duplicate detection in `validate-catalog`.

   The release tool already has the catalog context needed to reject duplicate implementation revisions. Keeping that validation as the final guard avoids splitting catalog identity rules across GitHub Actions expressions and Python code.

3. Run catalog validation before any expensive or irreversible release work.

   The workflow already validates catalog compatibility before worker package tests, Docker builds, and registry publication. The implementation should preserve that ordering and add tests or dry-run coverage so duplicate manual inputs fail early.

4. Document the operator contract.

   Operators should know that each non-rollback manual release needs a new implementation revision not present in the selected runtime contract. Rollback remains a catalog-default change to a previously published immutable revision, not reusing a duplicate revision in the release workflow.

## Risks / Trade-offs

- Manual entry can still contain typos or inconsistent naming -> Mitigate with the existing duplicate validation now, and optionally add revision format validation later if release naming needs to be standardized.
- Removing the default adds one required operator step -> Acceptable because a fixed default is unsafe for an immutable release identifier.
- Tag-triggered releases can still collide if the tag name is reused or already cataloged -> Mitigate by preserving catalog validation for all triggers before build or publish.
