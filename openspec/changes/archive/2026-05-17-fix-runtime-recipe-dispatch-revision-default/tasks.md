## 1. Workflow Input

- [x] 1.1 Remove the known-duplicate `implementation_revision` default from `.github/workflows/deploy-runtime-recipe.yml`.
- [x] 1.2 Confirm manual dispatch still requires operators to provide an implementation revision.

## 2. Early Duplicate Validation

- [x] 2.1 Preserve catalog compatibility validation before worker validation, Docker builds, registry publication, and Runtime Catalog PR creation.
- [x] 2.2 Add or update release-tool/workflow coverage proving `2026.05.16-001` is rejected as an existing implementation revision before build or publish steps.
- [x] 2.3 Add or update release-tool/workflow coverage proving a fresh implementation revision passes catalog duplicate validation for the selected recipe.

## 3. Documentation

- [x] 3.1 Update `workers/provisioner/DEPLOYMENT.md` to explain that manual releases require a new non-existing implementation revision.
- [x] 3.2 Document that rollback uses Runtime Catalog default selection rather than reusing an existing implementation revision in the release workflow.

## 4. Verification

- [x] 4.1 Run targeted runtime recipe release-tool tests or dry-run validation.
- [x] 4.2 Run formatting or linting for any changed workflow/tooling/documentation files where applicable.
