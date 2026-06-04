use crate::domain::{provider::GpuCloudProviderId, shared::ApiKeySetup};

use super::{
    error::ProviderApiKeyError,
    provider::ProviderIdentityValidator,
    store::{ProviderApiKey, ProviderApiKeyStore},
};

pub struct ProviderApiKeyService<S, V> {
    store: S,
    validator: V,
}

impl<S, V> ProviderApiKeyService<S, V>
where
    S: ProviderApiKeyStore,
    V: ProviderIdentityValidator,
{
    pub fn new(store: S, validator: V) -> Self {
        Self { store, validator }
    }

    pub async fn has_key(
        &self,
        provider_id: GpuCloudProviderId,
    ) -> Result<bool, ProviderApiKeyError> {
        self.store.has_key(provider_id).await
    }

    pub async fn read_key(
        &self,
        provider_id: GpuCloudProviderId,
    ) -> Result<ProviderApiKey, ProviderApiKeyError> {
        self.store
            .read_key(provider_id)
            .await?
            .ok_or(ProviderApiKeyError::ProviderSetupIncomplete)
    }

    pub async fn write_key(
        &self,
        provider_id: GpuCloudProviderId,
        api_key: ProviderApiKey,
    ) -> Result<ApiKeySetup, ProviderApiKeyError> {
        if self.store.has_key(provider_id).await? {
            return Err(ProviderApiKeyError::ProviderSetupAlreadyExists);
        }

        let setup = self
            .validator
            .validate_identity(provider_id, &api_key)
            .await?;
        let setup = validate_api_key_setup(setup)?;

        self.store.write_key(provider_id, &api_key).await?;

        Ok(setup)
    }

    pub async fn remove_key(
        &self,
        provider_id: GpuCloudProviderId,
    ) -> Result<(), ProviderApiKeyError> {
        if !self.store.has_key(provider_id).await? {
            return Err(ProviderApiKeyError::ProviderSetupIncomplete);
        }

        self.store.remove_key(provider_id).await
    }

    pub async fn validate_identity(
        &self,
        provider_id: GpuCloudProviderId,
    ) -> Result<ApiKeySetup, ProviderApiKeyError> {
        let api_key = self.read_key(provider_id).await?;
        let setup = self
            .validator
            .validate_identity(provider_id, &api_key)
            .await?;

        validate_api_key_setup(setup)
    }
}

fn validate_api_key_setup(setup: ApiKeySetup) -> Result<ApiKeySetup, ProviderApiKeyError> {
    if setup.email.trim().is_empty()
        || setup.username.trim().is_empty()
        || setup.key_display_name.trim().is_empty()
        || setup.email.chars().any(char::is_control)
        || setup.username.chars().any(char::is_control)
        || setup.key_display_name.chars().any(char::is_control)
    {
        return Err(ProviderApiKeyError::ProviderIdentityResponseInvalid);
    }

    Ok(setup)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use crate::shared::AppFuture;

    use super::*;

    #[derive(Default)]
    struct StoreState {
        keys: HashMap<GpuCloudProviderId, ProviderApiKey>,
        calls: Vec<&'static str>,
        has_key_error: Option<ProviderApiKeyError>,
        read_key_error: Option<ProviderApiKeyError>,
        write_key_error: Option<ProviderApiKeyError>,
        remove_key_error: Option<ProviderApiKeyError>,
    }

    struct FakeStore {
        state: Arc<Mutex<StoreState>>,
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    impl FakeStore {
        fn new(state: Arc<Mutex<StoreState>>, events: Arc<Mutex<Vec<&'static str>>>) -> Self {
            Self { state, events }
        }
    }

    impl ProviderApiKeyStore for FakeStore {
        fn has_key<'a>(
            &'a self,
            provider_id: GpuCloudProviderId,
        ) -> AppFuture<'a, Result<bool, ProviderApiKeyError>> {
            Box::pin(async move {
                self.events
                    .lock()
                    .expect("events lock should succeed")
                    .push("has_key");
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("has_key");

                if let Some(error) = state.has_key_error.clone() {
                    return Err(error);
                }

                Ok(state.keys.contains_key(&provider_id))
            })
        }

        fn read_key<'a>(
            &'a self,
            provider_id: GpuCloudProviderId,
        ) -> AppFuture<'a, Result<Option<ProviderApiKey>, ProviderApiKeyError>> {
            Box::pin(async move {
                self.events
                    .lock()
                    .expect("events lock should succeed")
                    .push("read_key");
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("read_key");

                if let Some(error) = state.read_key_error.clone() {
                    return Err(error);
                }

                Ok(state.keys.get(&provider_id).cloned())
            })
        }

        fn write_key<'a>(
            &'a self,
            provider_id: GpuCloudProviderId,
            api_key: &'a ProviderApiKey,
        ) -> AppFuture<'a, Result<(), ProviderApiKeyError>> {
            Box::pin(async move {
                self.events
                    .lock()
                    .expect("events lock should succeed")
                    .push("write_key");
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("write_key");

                if let Some(error) = state.write_key_error.clone() {
                    return Err(error);
                }

                state.keys.insert(provider_id, api_key.clone());
                Ok(())
            })
        }

        fn remove_key<'a>(
            &'a self,
            provider_id: GpuCloudProviderId,
        ) -> AppFuture<'a, Result<(), ProviderApiKeyError>> {
            Box::pin(async move {
                self.events
                    .lock()
                    .expect("events lock should succeed")
                    .push("remove_key");
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("remove_key");

                if let Some(error) = state.remove_key_error.clone() {
                    return Err(error);
                }

                state.keys.remove(&provider_id);
                Ok(())
            })
        }
    }

    struct ValidatorState {
        calls: Vec<&'static str>,
        last_provider_id: Option<GpuCloudProviderId>,
        last_api_key: Option<String>,
        result: Result<ApiKeySetup, ProviderApiKeyError>,
    }

    struct FakeValidator {
        state: Arc<Mutex<ValidatorState>>,
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    type TestService = ProviderApiKeyService<FakeStore, FakeValidator>;
    type StoreStateHandle = Arc<Mutex<StoreState>>;
    type ValidatorStateHandle = Arc<Mutex<ValidatorState>>;
    type EventLog = Arc<Mutex<Vec<&'static str>>>;

    impl FakeValidator {
        fn new(state: Arc<Mutex<ValidatorState>>, events: Arc<Mutex<Vec<&'static str>>>) -> Self {
            Self { state, events }
        }
    }

    impl ProviderIdentityValidator for FakeValidator {
        fn validate_identity<'a>(
            &'a self,
            provider_id: GpuCloudProviderId,
            api_key: &'a ProviderApiKey,
        ) -> AppFuture<'a, Result<ApiKeySetup, ProviderApiKeyError>> {
            Box::pin(async move {
                self.events
                    .lock()
                    .expect("events lock should succeed")
                    .push("validate_identity");
                let mut state = self.state.lock().expect("state lock should succeed");
                state.calls.push("validate_identity");
                state.last_provider_id = Some(provider_id);
                state.last_api_key = Some(api_key.expose_secret().to_string());

                state.result.clone()
            })
        }
    }

    fn setup() -> ApiKeySetup {
        ApiKeySetup {
            email: "user@example.test".to_string(),
            username: "user".to_string(),
            key_display_name: "default".to_string(),
        }
    }

    fn api_key() -> ProviderApiKey {
        ProviderApiKey::new("not-a-real-provider-key").expect("test key should be valid")
    }

    fn service_with_state(
        store_state: Arc<Mutex<StoreState>>,
        validator_state: Arc<Mutex<ValidatorState>>,
    ) -> ProviderApiKeyService<FakeStore, FakeValidator> {
        let events = Arc::new(Mutex::new(Vec::new()));
        ProviderApiKeyService::new(
            FakeStore::new(store_state, Arc::clone(&events)),
            FakeValidator::new(validator_state, events),
        )
    }

    fn service_parts() -> (TestService, StoreStateHandle, ValidatorStateHandle) {
        let store_state = Arc::new(Mutex::new(StoreState::default()));
        let validator_state = Arc::new(Mutex::new(ValidatorState {
            calls: Vec::new(),
            last_provider_id: None,
            last_api_key: None,
            result: Ok(setup()),
        }));
        let service = service_with_state(Arc::clone(&store_state), Arc::clone(&validator_state));

        (service, store_state, validator_state)
    }

    fn service_parts_with_events() -> (
        TestService,
        StoreStateHandle,
        ValidatorStateHandle,
        EventLog,
    ) {
        let store_state = Arc::new(Mutex::new(StoreState::default()));
        let validator_state = Arc::new(Mutex::new(ValidatorState {
            calls: Vec::new(),
            last_provider_id: None,
            last_api_key: None,
            result: Ok(setup()),
        }));
        let events = Arc::new(Mutex::new(Vec::new()));
        let service = ProviderApiKeyService::new(
            FakeStore::new(Arc::clone(&store_state), Arc::clone(&events)),
            FakeValidator::new(Arc::clone(&validator_state), Arc::clone(&events)),
        );

        (service, store_state, validator_state, events)
    }

    fn insert_key(state: &Arc<Mutex<StoreState>>) {
        state
            .lock()
            .expect("state lock should succeed")
            .keys
            .insert(GpuCloudProviderId::Runpod, api_key());
    }

    #[test]
    fn validate_api_key_setup_accepts_non_blank_fields() {
        assert_eq!(validate_api_key_setup(setup()), Ok(setup()));
    }

    #[test]
    fn validate_api_key_setup_rejects_blank_fields() {
        for invalid_setup in [
            ApiKeySetup {
                email: " ".to_string(),
                ..setup()
            },
            ApiKeySetup {
                username: " ".to_string(),
                ..setup()
            },
            ApiKeySetup {
                key_display_name: " ".to_string(),
                ..setup()
            },
        ] {
            assert_eq!(
                validate_api_key_setup(invalid_setup),
                Err(ProviderApiKeyError::ProviderIdentityResponseInvalid)
            );
        }
    }

    #[test]
    fn validate_api_key_setup_rejects_control_characters() {
        for invalid_setup in [
            ApiKeySetup {
                email: "user\n@example.test".to_string(),
                ..setup()
            },
            ApiKeySetup {
                username: "user\tname".to_string(),
                ..setup()
            },
            ApiKeySetup {
                key_display_name: "default\rkey".to_string(),
                ..setup()
            },
        ] {
            assert_eq!(
                validate_api_key_setup(invalid_setup),
                Err(ProviderApiKeyError::ProviderIdentityResponseInvalid)
            );
        }
    }

    #[test]
    fn has_key_returns_true_from_storage() {
        let (service, store_state, _) = service_parts();
        insert_key(&store_state);

        let has_key =
            block_on(service.has_key(GpuCloudProviderId::Runpod)).expect("has_key should succeed");

        assert!(has_key);
    }

    #[test]
    fn has_key_returns_false_from_storage() {
        let (service, _, _) = service_parts();

        let has_key =
            block_on(service.has_key(GpuCloudProviderId::Runpod)).expect("has_key should succeed");

        assert!(!has_key);
    }

    #[test]
    fn has_key_maps_storage_failure() {
        let (service, store_state, _) = service_parts();
        store_state
            .lock()
            .expect("state lock should succeed")
            .has_key_error = Some(ProviderApiKeyError::SecureKeyringUnavailable);

        let error = block_on(service.has_key(GpuCloudProviderId::Runpod))
            .expect_err("storage failure should be returned");

        assert_eq!(error, ProviderApiKeyError::SecureKeyringUnavailable);
    }

    #[test]
    fn has_key_does_not_call_provider_identity_validation() {
        let (service, _, validator_state) = service_parts();

        block_on(service.has_key(GpuCloudProviderId::Runpod)).expect("has_key should succeed");

        assert!(validator_state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn read_key_returns_stored_key() {
        let (service, store_state, _) = service_parts();
        insert_key(&store_state);

        let stored_key = block_on(service.read_key(GpuCloudProviderId::Runpod))
            .expect("stored key should be returned");

        assert_eq!(stored_key.expose_secret(), "not-a-real-provider-key");
    }

    #[test]
    fn read_key_returns_incomplete_when_key_is_missing() {
        let (service, _, _) = service_parts();

        let error = block_on(service.read_key(GpuCloudProviderId::Runpod))
            .expect_err("missing key should be incomplete setup");

        assert_eq!(error, ProviderApiKeyError::ProviderSetupIncomplete);
    }

    #[test]
    fn read_key_maps_invalid_stored_key() {
        let (service, store_state, _) = service_parts();
        store_state
            .lock()
            .expect("state lock should succeed")
            .read_key_error = Some(ProviderApiKeyError::StoredProviderApiKeyInvalid);

        let error = block_on(service.read_key(GpuCloudProviderId::Runpod))
            .expect_err("invalid stored key should be returned");

        assert_eq!(error, ProviderApiKeyError::StoredProviderApiKeyInvalid);
    }

    #[test]
    fn read_key_does_not_call_provider_identity_validation() {
        let (service, store_state, validator_state) = service_parts();
        insert_key(&store_state);

        block_on(service.read_key(GpuCloudProviderId::Runpod)).expect("read_key should succeed");

        assert!(validator_state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn write_key_rejects_existing_provider_setup_before_validation() {
        let (service, store_state, validator_state, events) = service_parts_with_events();
        insert_key(&store_state);

        let error = block_on(service.write_key(GpuCloudProviderId::Runpod, api_key()))
            .expect_err("existing setup should be rejected");

        assert_eq!(error, ProviderApiKeyError::ProviderSetupAlreadyExists);
        assert!(validator_state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
        assert_eq!(
            *events.lock().expect("events lock should succeed"),
            vec!["has_key"]
        );
    }

    #[test]
    fn write_key_validates_provider_identity_before_storing_key() {
        let (service, _, _, events) = service_parts_with_events();

        block_on(service.write_key(GpuCloudProviderId::Runpod, api_key()))
            .expect("write_key should succeed");

        assert_eq!(
            *events.lock().expect("events lock should succeed"),
            vec!["has_key", "validate_identity", "write_key"]
        );
    }

    #[test]
    fn write_key_stores_non_blank_key_after_validation_succeeds() {
        let (service, store_state, _) = service_parts();

        block_on(service.write_key(GpuCloudProviderId::Runpod, api_key()))
            .expect("write_key should succeed");

        assert_eq!(
            store_state
                .lock()
                .expect("state lock should succeed")
                .keys
                .get(&GpuCloudProviderId::Runpod)
                .expect("key should be stored")
                .expose_secret(),
            "not-a-real-provider-key"
        );
    }

    #[test]
    fn write_key_returns_validated_setup_after_storing() {
        let (service, _, _) = service_parts();

        let returned_setup = block_on(service.write_key(GpuCloudProviderId::Runpod, api_key()))
            .expect("write_key should succeed");

        assert_eq!(returned_setup, setup());
    }

    #[test]
    fn blank_raw_key_input_cannot_be_written() {
        let error =
            ProviderApiKey::new(" ").expect_err("blank key construction should fail before write");

        assert_eq!(error, ProviderApiKeyError::StoredProviderApiKeyInvalid);
    }

    #[test]
    fn write_key_maps_storage_write_failure() {
        let (service, store_state, _) = service_parts();
        store_state
            .lock()
            .expect("state lock should succeed")
            .write_key_error = Some(ProviderApiKeyError::SecureKeyringUnavailable);

        let error = block_on(service.write_key(GpuCloudProviderId::Runpod, api_key()))
            .expect_err("storage failure should be returned");

        assert_eq!(error, ProviderApiKeyError::SecureKeyringUnavailable);
    }

    #[test]
    fn write_key_maps_storage_check_failure_without_validating_identity() {
        let (service, store_state, validator_state) = service_parts();
        store_state
            .lock()
            .expect("state lock should succeed")
            .has_key_error = Some(ProviderApiKeyError::SecureKeyringUnavailable);

        let error = block_on(service.write_key(GpuCloudProviderId::Runpod, api_key()))
            .expect_err("has_key failure should be returned");

        assert_eq!(error, ProviderApiKeyError::SecureKeyringUnavailable);
        assert!(validator_state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn write_key_maps_provider_failures_without_mutating_storage() {
        for provider_error in [
            ProviderApiKeyError::ProviderUnauthorized,
            ProviderApiKeyError::ProviderRateLimited,
            ProviderApiKeyError::ProviderTimeout,
            ProviderApiKeyError::ProviderRequestFailed {
                message: "provider request failed".to_string(),
            },
        ] {
            let (service, store_state, validator_state) = service_parts();
            validator_state
                .lock()
                .expect("state lock should succeed")
                .result = Err(provider_error.clone());

            let error = block_on(service.write_key(GpuCloudProviderId::Runpod, api_key()))
                .expect_err("provider validation failure should be returned");

            assert_eq!(error, provider_error);
            assert!(store_state
                .lock()
                .expect("state lock should succeed")
                .keys
                .is_empty());
        }
    }

    #[test]
    fn write_key_rejects_invalid_setup_without_mutating_storage() {
        for invalid_setup in [
            ApiKeySetup {
                email: " ".to_string(),
                ..setup()
            },
            ApiKeySetup {
                username: "user\nname".to_string(),
                ..setup()
            },
        ] {
            let (service, store_state, validator_state) = service_parts();
            validator_state
                .lock()
                .expect("state lock should succeed")
                .result = Ok(invalid_setup);

            let error = block_on(service.write_key(GpuCloudProviderId::Runpod, api_key()))
                .expect_err("invalid setup should be rejected");

            assert_eq!(error, ProviderApiKeyError::ProviderIdentityResponseInvalid);
            assert!(store_state
                .lock()
                .expect("state lock should succeed")
                .keys
                .is_empty());
        }
    }

    #[test]
    fn remove_key_removes_existing_provider_api_key() {
        let (service, store_state, _) = service_parts();
        insert_key(&store_state);

        block_on(service.remove_key(GpuCloudProviderId::Runpod))
            .expect("existing key should be removed");

        assert!(!store_state
            .lock()
            .expect("state lock should succeed")
            .keys
            .contains_key(&GpuCloudProviderId::Runpod));
    }

    #[test]
    fn remove_key_returns_incomplete_when_no_key_exists() {
        let (service, _, _) = service_parts();

        let error = block_on(service.remove_key(GpuCloudProviderId::Runpod))
            .expect_err("missing key should be incomplete setup");

        assert_eq!(error, ProviderApiKeyError::ProviderSetupIncomplete);
    }

    #[test]
    fn remove_key_maps_storage_remove_or_check_failure() {
        let (service, store_state, _) = service_parts();
        store_state
            .lock()
            .expect("state lock should succeed")
            .has_key_error = Some(ProviderApiKeyError::SecureKeyringUnavailable);

        let error = block_on(service.remove_key(GpuCloudProviderId::Runpod))
            .expect_err("has_key failure should be returned");

        assert_eq!(error, ProviderApiKeyError::SecureKeyringUnavailable);

        let (service, store_state, _) = service_parts();
        insert_key(&store_state);
        store_state
            .lock()
            .expect("state lock should succeed")
            .remove_key_error = Some(ProviderApiKeyError::SecureKeyringUnavailable);

        let error = block_on(service.remove_key(GpuCloudProviderId::Runpod))
            .expect_err("remove failure should be returned");

        assert_eq!(error, ProviderApiKeyError::SecureKeyringUnavailable);
    }

    #[test]
    fn remove_key_does_not_call_provider_identity_validation() {
        let (service, store_state, validator_state) = service_parts();
        insert_key(&store_state);

        block_on(service.remove_key(GpuCloudProviderId::Runpod))
            .expect("remove_key should succeed");

        assert!(validator_state
            .lock()
            .expect("state lock should succeed")
            .calls
            .is_empty());
    }

    #[test]
    fn validate_identity_validates_with_stored_key() {
        let (service, store_state, validator_state) = service_parts();
        insert_key(&store_state);

        block_on(service.validate_identity(GpuCloudProviderId::Runpod))
            .expect("validation should succeed");

        let validator_state = validator_state.lock().expect("state lock should succeed");
        assert_eq!(
            validator_state.last_provider_id,
            Some(GpuCloudProviderId::Runpod)
        );
        assert_eq!(
            validator_state.last_api_key,
            Some("not-a-real-provider-key".to_string())
        );
    }

    #[test]
    fn validate_identity_returns_api_key_setup() {
        let (service, store_state, _) = service_parts();
        insert_key(&store_state);

        let returned_setup = block_on(service.validate_identity(GpuCloudProviderId::Runpod))
            .expect("validation should succeed");

        assert_eq!(returned_setup, setup());
    }

    #[test]
    fn validate_identity_returns_incomplete_when_key_is_missing() {
        let (service, _, _) = service_parts();

        let error = block_on(service.validate_identity(GpuCloudProviderId::Runpod))
            .expect_err("missing key should be incomplete setup");

        assert_eq!(error, ProviderApiKeyError::ProviderSetupIncomplete);
    }

    #[test]
    fn validate_identity_maps_provider_failures() {
        for provider_error in [
            ProviderApiKeyError::ProviderUnauthorized,
            ProviderApiKeyError::ProviderRateLimited,
            ProviderApiKeyError::ProviderTimeout,
            ProviderApiKeyError::ProviderRequestFailed {
                message: "provider request failed".to_string(),
            },
        ] {
            let (service, store_state, validator_state) = service_parts();
            insert_key(&store_state);
            validator_state
                .lock()
                .expect("state lock should succeed")
                .result = Err(provider_error.clone());

            let error = block_on(service.validate_identity(GpuCloudProviderId::Runpod))
                .expect_err("provider validation failure should be returned");

            assert_eq!(error, provider_error);
        }
    }

    #[test]
    fn validate_identity_rejects_invalid_setup() {
        for invalid_setup in [
            ApiKeySetup {
                email: " ".to_string(),
                ..setup()
            },
            ApiKeySetup {
                key_display_name: "default\nkey".to_string(),
                ..setup()
            },
        ] {
            let (service, store_state, validator_state) = service_parts();
            insert_key(&store_state);
            validator_state
                .lock()
                .expect("state lock should succeed")
                .result = Ok(invalid_setup);

            let error = block_on(service.validate_identity(GpuCloudProviderId::Runpod))
                .expect_err("invalid setup should be rejected");

            assert_eq!(error, ProviderApiKeyError::ProviderIdentityResponseInvalid);
        }
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        use std::{
            future::Future,
            pin::Pin,
            task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
        };

        fn raw_waker() -> RawWaker {
            fn clone(_: *const ()) -> RawWaker {
                raw_waker()
            }
            fn wake(_: *const ()) {}
            fn wake_by_ref(_: *const ()) {}
            fn drop(_: *const ()) {}

            RawWaker::new(
                std::ptr::null(),
                &RawWakerVTable::new(clone, wake, wake_by_ref, drop),
            )
        }

        let waker = unsafe { Waker::from_raw(raw_waker()) };
        let mut context = Context::from_waker(&waker);
        let mut future = Box::pin(future);

        loop {
            match Pin::new(&mut future).poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => {}
            }
        }
    }
}
