## GPU Cloud Provider

An external infrastructure provider that gives the user access to remote GPU compute, Persistent Storage Volume, and serverless inference infrastructure (e.g. Serverless Endpoint).

Examples:

- RunPod
- Vast
- other GPU cloud providers

In v1, only RunPod is supported.

**Invariants:**

- The user pays the GPU Cloud Provider directly.
- LumaForge does not resell or proxy GPU Cloud Provider billing in v1.

## See Also

- [Provider API Key](./provider-api-key.md)
- [Provider Resource](./provider-resource.md)
- [Persistent Storage Volume](./persistent-storage-volume.md)
- [Serverless Endpoint](./serverless-endpoint.md)
- [Provisioning Pod](./provisioning-pod.md)
