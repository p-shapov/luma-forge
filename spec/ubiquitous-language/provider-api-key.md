## Provider API Key

A user-provided secret credential used by LumaForge to manage Provider Resources inside the user's GPU Cloud Provider account.

**Invariants:**

- The Provider API Key is stored locally in a secure keyring.
- The Provider API Key is used by the Native Layer (Rust/Tauri) to communicate with the GPU Cloud Provider.
- The Provider API Key must not be present in local metadata and logs.
- The Provider API Key must not be exposed to Client (React).

## See Also

- [GPU Cloud Provider](./gpu-cloud-provider.md)
- [Provider Resource](./provider-resource.md)