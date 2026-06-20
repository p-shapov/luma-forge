use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{secrets::SecretsStorageError, shared::ApiError};

macro_rules! define_setup_api_key_error_code {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            #[error("native initialization failed")]
            NativeInitializationFailed,
            #[error("api key is required")]
            SecretRequired,
            #[error("api key is already configured")]
            KeyAlreadyExists,
            #[error("secure storage is unavailable")]
            StoreUnavailable,
            #[error("api key identity request was unauthorized")]
            IdentityUnauthorized,
            #[error("api key identity request has insufficient permissions")]
            IdentityInsufficientPermissions,
            #[error("api key identity request was rate limited")]
            IdentityRateLimited,
            #[error("api key identity request timed out")]
            IdentityTimeout,
            #[error("api key identity request failed")]
            IdentityRequestFailed,
            #[error("api key identity response is invalid")]
            IdentityResponseInvalid,
        }
    };
}

macro_rules! define_get_api_key_identity_error_code {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            #[error("native initialization failed")]
            NativeInitializationFailed,
            #[error("api key is not configured")]
            KeyNotFound,
            #[error("secure storage is unavailable")]
            StoreUnavailable,
            #[error("stored api key is invalid")]
            StoredSecretInvalid,
            #[error("api key identity request was unauthorized")]
            IdentityUnauthorized,
            #[error("api key identity request has insufficient permissions")]
            IdentityInsufficientPermissions,
            #[error("api key identity request was rate limited")]
            IdentityRateLimited,
            #[error("api key identity request timed out")]
            IdentityTimeout,
            #[error("api key identity request failed")]
            IdentityRequestFailed,
            #[error("api key identity response is invalid")]
            IdentityResponseInvalid,
        }
    };
}

macro_rules! define_delete_api_key_error_code {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            #[error("native initialization failed")]
            NativeInitializationFailed,
            #[error("api key is not configured")]
            KeyNotFound,
            #[error("secure storage is unavailable")]
            StoreUnavailable,
        }
    };
}

define_setup_api_key_error_code!(SetupRunpodApiKeyErrorCode);
define_get_api_key_identity_error_code!(GetRunpodApiKeyIdentityErrorCode);
define_delete_api_key_error_code!(DeleteRunpodApiKeyErrorCode);

define_setup_api_key_error_code!(SetupHuggingFaceApiKeyErrorCode);
define_get_api_key_identity_error_code!(GetHuggingFaceApiKeyIdentityErrorCode);
define_delete_api_key_error_code!(DeleteHuggingFaceApiKeyErrorCode);

pub fn setup_runpod_api_key_error(error: &SecretsStorageError) -> SetupRunpodApiKeyErrorCode {
    match setup_api_key_error_kind(error) {
        SetupApiKeyErrorKind::SecretRequired => SetupRunpodApiKeyErrorCode::SecretRequired,
        SetupApiKeyErrorKind::KeyAlreadyExists => SetupRunpodApiKeyErrorCode::KeyAlreadyExists,
        SetupApiKeyErrorKind::StoreUnavailable => SetupRunpodApiKeyErrorCode::StoreUnavailable,
        SetupApiKeyErrorKind::IdentityUnauthorized => {
            SetupRunpodApiKeyErrorCode::IdentityUnauthorized
        }
        SetupApiKeyErrorKind::IdentityInsufficientPermissions => {
            SetupRunpodApiKeyErrorCode::IdentityInsufficientPermissions
        }
        SetupApiKeyErrorKind::IdentityRateLimited => {
            SetupRunpodApiKeyErrorCode::IdentityRateLimited
        }
        SetupApiKeyErrorKind::IdentityTimeout => SetupRunpodApiKeyErrorCode::IdentityTimeout,
        SetupApiKeyErrorKind::IdentityRequestFailed => {
            SetupRunpodApiKeyErrorCode::IdentityRequestFailed
        }
        SetupApiKeyErrorKind::IdentityResponseInvalid => {
            SetupRunpodApiKeyErrorCode::IdentityResponseInvalid
        }
    }
}

pub fn get_runpod_api_key_identity_error(
    error: &SecretsStorageError,
) -> GetRunpodApiKeyIdentityErrorCode {
    match get_api_key_identity_error_kind(error) {
        GetApiKeyIdentityErrorKind::KeyNotFound => GetRunpodApiKeyIdentityErrorCode::KeyNotFound,
        GetApiKeyIdentityErrorKind::StoreUnavailable => {
            GetRunpodApiKeyIdentityErrorCode::StoreUnavailable
        }
        GetApiKeyIdentityErrorKind::StoredSecretInvalid => {
            GetRunpodApiKeyIdentityErrorCode::StoredSecretInvalid
        }
        GetApiKeyIdentityErrorKind::IdentityUnauthorized => {
            GetRunpodApiKeyIdentityErrorCode::IdentityUnauthorized
        }
        GetApiKeyIdentityErrorKind::IdentityInsufficientPermissions => {
            GetRunpodApiKeyIdentityErrorCode::IdentityInsufficientPermissions
        }
        GetApiKeyIdentityErrorKind::IdentityRateLimited => {
            GetRunpodApiKeyIdentityErrorCode::IdentityRateLimited
        }
        GetApiKeyIdentityErrorKind::IdentityTimeout => {
            GetRunpodApiKeyIdentityErrorCode::IdentityTimeout
        }
        GetApiKeyIdentityErrorKind::IdentityRequestFailed => {
            GetRunpodApiKeyIdentityErrorCode::IdentityRequestFailed
        }
        GetApiKeyIdentityErrorKind::IdentityResponseInvalid => {
            GetRunpodApiKeyIdentityErrorCode::IdentityResponseInvalid
        }
    }
}

pub fn delete_runpod_api_key_error(error: &SecretsStorageError) -> DeleteRunpodApiKeyErrorCode {
    match delete_api_key_error_kind(error) {
        DeleteApiKeyErrorKind::KeyNotFound => DeleteRunpodApiKeyErrorCode::KeyNotFound,
        DeleteApiKeyErrorKind::StoreUnavailable => DeleteRunpodApiKeyErrorCode::StoreUnavailable,
    }
}

pub fn setup_hugging_face_api_key_error(
    error: &SecretsStorageError,
) -> SetupHuggingFaceApiKeyErrorCode {
    match setup_api_key_error_kind(error) {
        SetupApiKeyErrorKind::SecretRequired => SetupHuggingFaceApiKeyErrorCode::SecretRequired,
        SetupApiKeyErrorKind::KeyAlreadyExists => SetupHuggingFaceApiKeyErrorCode::KeyAlreadyExists,
        SetupApiKeyErrorKind::StoreUnavailable => SetupHuggingFaceApiKeyErrorCode::StoreUnavailable,
        SetupApiKeyErrorKind::IdentityUnauthorized => {
            SetupHuggingFaceApiKeyErrorCode::IdentityUnauthorized
        }
        SetupApiKeyErrorKind::IdentityInsufficientPermissions => {
            SetupHuggingFaceApiKeyErrorCode::IdentityInsufficientPermissions
        }
        SetupApiKeyErrorKind::IdentityRateLimited => {
            SetupHuggingFaceApiKeyErrorCode::IdentityRateLimited
        }
        SetupApiKeyErrorKind::IdentityTimeout => SetupHuggingFaceApiKeyErrorCode::IdentityTimeout,
        SetupApiKeyErrorKind::IdentityRequestFailed => {
            SetupHuggingFaceApiKeyErrorCode::IdentityRequestFailed
        }
        SetupApiKeyErrorKind::IdentityResponseInvalid => {
            SetupHuggingFaceApiKeyErrorCode::IdentityResponseInvalid
        }
    }
}

pub fn get_hugging_face_api_key_identity_error(
    error: &SecretsStorageError,
) -> GetHuggingFaceApiKeyIdentityErrorCode {
    match get_api_key_identity_error_kind(error) {
        GetApiKeyIdentityErrorKind::KeyNotFound => {
            GetHuggingFaceApiKeyIdentityErrorCode::KeyNotFound
        }
        GetApiKeyIdentityErrorKind::StoreUnavailable => {
            GetHuggingFaceApiKeyIdentityErrorCode::StoreUnavailable
        }
        GetApiKeyIdentityErrorKind::StoredSecretInvalid => {
            GetHuggingFaceApiKeyIdentityErrorCode::StoredSecretInvalid
        }
        GetApiKeyIdentityErrorKind::IdentityUnauthorized => {
            GetHuggingFaceApiKeyIdentityErrorCode::IdentityUnauthorized
        }
        GetApiKeyIdentityErrorKind::IdentityInsufficientPermissions => {
            GetHuggingFaceApiKeyIdentityErrorCode::IdentityInsufficientPermissions
        }
        GetApiKeyIdentityErrorKind::IdentityRateLimited => {
            GetHuggingFaceApiKeyIdentityErrorCode::IdentityRateLimited
        }
        GetApiKeyIdentityErrorKind::IdentityTimeout => {
            GetHuggingFaceApiKeyIdentityErrorCode::IdentityTimeout
        }
        GetApiKeyIdentityErrorKind::IdentityRequestFailed => {
            GetHuggingFaceApiKeyIdentityErrorCode::IdentityRequestFailed
        }
        GetApiKeyIdentityErrorKind::IdentityResponseInvalid => {
            GetHuggingFaceApiKeyIdentityErrorCode::IdentityResponseInvalid
        }
    }
}

pub fn delete_hugging_face_api_key_error(
    error: &SecretsStorageError,
) -> DeleteHuggingFaceApiKeyErrorCode {
    match delete_api_key_error_kind(error) {
        DeleteApiKeyErrorKind::KeyNotFound => DeleteHuggingFaceApiKeyErrorCode::KeyNotFound,
        DeleteApiKeyErrorKind::StoreUnavailable => {
            DeleteHuggingFaceApiKeyErrorCode::StoreUnavailable
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SetupApiKeyErrorKind {
    SecretRequired,
    KeyAlreadyExists,
    StoreUnavailable,
    IdentityUnauthorized,
    IdentityInsufficientPermissions,
    IdentityRateLimited,
    IdentityTimeout,
    IdentityRequestFailed,
    IdentityResponseInvalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GetApiKeyIdentityErrorKind {
    KeyNotFound,
    StoreUnavailable,
    StoredSecretInvalid,
    IdentityUnauthorized,
    IdentityInsufficientPermissions,
    IdentityRateLimited,
    IdentityTimeout,
    IdentityRequestFailed,
    IdentityResponseInvalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeleteApiKeyErrorKind {
    KeyNotFound,
    StoreUnavailable,
}

fn setup_api_key_error_kind(error: &SecretsStorageError) -> SetupApiKeyErrorKind {
    match error {
        SecretsStorageError::SecretRequired => SetupApiKeyErrorKind::SecretRequired,
        SecretsStorageError::KeyAlreadyExists => SetupApiKeyErrorKind::KeyAlreadyExists,
        SecretsStorageError::KeyNotFound => SetupApiKeyErrorKind::StoreUnavailable,
        SecretsStorageError::StoreUnavailable => SetupApiKeyErrorKind::StoreUnavailable,
        SecretsStorageError::StoredSecretInvalid => SetupApiKeyErrorKind::StoreUnavailable,
        SecretsStorageError::IdentityRequestFailed(error) => setup_identity_error(error),
        SecretsStorageError::IdentityResponseInvalid { .. } => {
            SetupApiKeyErrorKind::IdentityResponseInvalid
        }
    }
}

fn get_api_key_identity_error_kind(error: &SecretsStorageError) -> GetApiKeyIdentityErrorKind {
    match error {
        SecretsStorageError::SecretRequired => GetApiKeyIdentityErrorKind::KeyNotFound,
        SecretsStorageError::KeyAlreadyExists => GetApiKeyIdentityErrorKind::StoreUnavailable,
        SecretsStorageError::KeyNotFound => GetApiKeyIdentityErrorKind::KeyNotFound,
        SecretsStorageError::StoreUnavailable => GetApiKeyIdentityErrorKind::StoreUnavailable,
        SecretsStorageError::StoredSecretInvalid => GetApiKeyIdentityErrorKind::StoredSecretInvalid,
        SecretsStorageError::IdentityRequestFailed(error) => get_identity_error(error),
        SecretsStorageError::IdentityResponseInvalid { .. } => {
            GetApiKeyIdentityErrorKind::IdentityResponseInvalid
        }
    }
}

fn delete_api_key_error_kind(error: &SecretsStorageError) -> DeleteApiKeyErrorKind {
    match error {
        SecretsStorageError::KeyNotFound => DeleteApiKeyErrorKind::KeyNotFound,
        SecretsStorageError::SecretRequired
        | SecretsStorageError::KeyAlreadyExists
        | SecretsStorageError::StoreUnavailable
        | SecretsStorageError::StoredSecretInvalid
        | SecretsStorageError::IdentityRequestFailed(_)
        | SecretsStorageError::IdentityResponseInvalid { .. } => {
            DeleteApiKeyErrorKind::StoreUnavailable
        }
    }
}

fn setup_identity_error(error: &ApiError) -> SetupApiKeyErrorKind {
    match error {
        ApiError::Unauthorized => SetupApiKeyErrorKind::IdentityUnauthorized,
        ApiError::InsufficientPermissions => SetupApiKeyErrorKind::IdentityInsufficientPermissions,
        ApiError::RateLimited => SetupApiKeyErrorKind::IdentityRateLimited,
        ApiError::Timeout => SetupApiKeyErrorKind::IdentityTimeout,
        ApiError::RequestFailed { .. } => SetupApiKeyErrorKind::IdentityRequestFailed,
    }
}

fn get_identity_error(error: &ApiError) -> GetApiKeyIdentityErrorKind {
    match error {
        ApiError::Unauthorized => GetApiKeyIdentityErrorKind::IdentityUnauthorized,
        ApiError::InsufficientPermissions => {
            GetApiKeyIdentityErrorKind::IdentityInsufficientPermissions
        }
        ApiError::RateLimited => GetApiKeyIdentityErrorKind::IdentityRateLimited,
        ApiError::Timeout => GetApiKeyIdentityErrorKind::IdentityTimeout,
        ApiError::RequestFailed { .. } => GetApiKeyIdentityErrorKind::IdentityRequestFailed,
    }
}
