pub struct SecretsStorageService<S, I> {
    _store: S,
    _identity: I,
}

impl<S, I> SecretsStorageService<S, I> {
    pub fn new(store: S, identity: I) -> Self {
        Self {
            _store: store,
            _identity: identity,
        }
    }
}
