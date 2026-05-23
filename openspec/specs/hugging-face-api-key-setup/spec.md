# hugging-face-api-key-setup Specification

## Purpose
TBD - created by archiving change support-hugging-face-api-key. Update Purpose after archive.
## Requirements
### Requirement: Report Hugging Face API key setup status
The Native Layer SHALL expose a command that reports whether a Hugging Face API key is configured by reading the secure keyring and validating stored key identity with Hugging Face before returning setup details.

#### Scenario: Hugging Face API key setup is missing
- **WHEN** the Client requests Hugging Face API key setup status and no local keyring entry exists
- **THEN** the Native Layer SHALL return no configured setup
- **AND** the response MUST NOT contain a raw Hugging Face API key

#### Scenario: Stored Hugging Face API key is valid
- **WHEN** the Client requests Hugging Face API key setup status and a local keyring entry exists
- **AND** Hugging Face identity validation succeeds for the stored key
- **AND** the stored key has Hugging Face model-download capability flags
- **THEN** the Native Layer SHALL return configured setup with `token_name`, `user_name`, and `user_email`
- **AND** `token_name` SHALL equal the Hugging Face access-token display name
- **AND** `user_name` SHALL equal the Hugging Face user name
- **AND** `user_email` SHALL equal the Hugging Face user email when Hugging Face provides one
- **AND** the response MUST NOT contain the raw Hugging Face API key

#### Scenario: Stored Hugging Face API key is invalid
- **WHEN** the Client requests Hugging Face API key setup status and the stored key cannot be parsed, cannot authenticate, lacks model-download capability flags, or produces an invalid identity response
- **THEN** the Native Layer SHALL reject the request with a UI-safe Hugging Face setup error
- **AND** the generated command error MUST NOT include the stored keyring value, bearer headers, Hugging Face response bodies, or keyring internals

### Requirement: Create Hugging Face API key setup
The Native Layer SHALL expose a command that validates a submitted Hugging Face API key with Hugging Face identity lookup and stores it only in the secure keyring when validation succeeds.

#### Scenario: Submitted Hugging Face API key is valid
- **WHEN** the Client submits a non-empty Hugging Face API key
- **AND** Hugging Face identity validation returns a non-empty access-token display name and user name
- **AND** the submitted key has Hugging Face model-download capability flags
- **THEN** the Native Layer SHALL store the key in the secure keyring
- **AND** the Native Layer SHALL return configured setup with `token_name`, `user_name`, and `user_email`
- **AND** the response MUST NOT contain the raw Hugging Face API key

#### Scenario: Submitted Hugging Face API key is blank
- **WHEN** the Client submits a blank Hugging Face API key
- **THEN** the Native Layer SHALL reject the request with a UI-safe required-key error
- **AND** the Native Layer MUST NOT mutate the keyring

#### Scenario: Submitted Hugging Face API key is unauthorized
- **WHEN** Hugging Face rejects the submitted API key as unauthorized, forbidden, unauthenticated, inactive, missing model-download capability flags, or otherwise invalid for identity validation
- **THEN** the Native Layer SHALL reject the request with a UI-safe unauthorized-key error
- **AND** the Native Layer MUST NOT mutate the keyring
- **AND** the generated command error MUST NOT include the submitted Hugging Face API key

#### Scenario: Hugging Face identity response is invalid
- **WHEN** Hugging Face identity validation succeeds at the transport layer but the response lacks a non-empty access-token display name or user name
- **THEN** the Native Layer SHALL reject the request with a UI-safe invalid identity response error
- **AND** the Native Layer MUST NOT mutate the keyring

#### Scenario: Fine-grained token lacks model download flags
- **WHEN** Hugging Face identity validation returns a fine-grained token
- **AND** the token does not include `fineGrained.canReadGatedRepos` set to `true`
- **OR** the token does not include `repo.content.read` in global or scoped permissions
- **THEN** the Native Layer SHALL reject the request with a UI-safe unauthorized-key error
- **AND** the Native Layer MUST NOT mutate the keyring

#### Scenario: Secure keyring write fails
- **WHEN** the submitted Hugging Face API key validates successfully but secure keyring write fails
- **THEN** the Native Layer SHALL reject the request with a UI-safe secure keyring error
- **AND** the response MUST NOT contain the submitted Hugging Face API key

### Requirement: Delete Hugging Face API key setup
The Native Layer SHALL expose a command that deletes the stored Hugging Face API key from secure keyring storage.

#### Scenario: Hugging Face API key setup is deleted
- **WHEN** the Client requests deletion and a Hugging Face API key keyring entry exists
- **THEN** the Native Layer SHALL delete the local keyring entry
- **AND** the Native Layer SHALL return no configured setup

#### Scenario: Hugging Face API key setup is already missing
- **WHEN** the Client requests deletion and no Hugging Face API key keyring entry exists
- **THEN** the Native Layer SHALL reject the request with a UI-safe not-found error

#### Scenario: Delete keyring access fails
- **WHEN** the Client requests deletion and secure keyring entry lookup or deletion fails
- **THEN** the Native Layer SHALL reject the request with a UI-safe secure keyring error
- **AND** the generated command error MUST NOT include keyring internals

### Requirement: Keep Hugging Face API key secret from the Client
The Native Layer MUST NOT return, persist outside secure keyring, log, or include Hugging Face API keys in generated frontend types, command responses, command errors, workspace metadata, provider metadata, worker status, or test fixtures.

#### Scenario: Setup request debug output is rendered
- **WHEN** a Hugging Face setup request is formatted for debug output
- **THEN** the formatted output SHALL include a redacted marker for the API key field
- **AND** it MUST NOT include the raw Hugging Face API key value

#### Scenario: Setup identity is returned
- **WHEN** a Hugging Face setup command returns configured setup
- **THEN** it SHALL return only `token_name`, `user_name`, and `user_email`
- **AND** `token_name` MUST be the Hugging Face access-token display name rather than raw token material

