# Reference Sketches

This directory contains draft TypeScript sketches for early implementation planning.

They are not production source code, runtime contracts, or a guarantee that the final implementation must match them field-for-field. Use them as naming and boundary references while implementing the project. The high-level specs remain the conceptual source of truth.

`entities/` contains draft entity sketches.

`shared/` contains generic reusable type sketches without domain-specific meaning.

`native-contracts.ts` sketches the planned React -> Rust/Tauri command boundary. It is still reference material, but should be treated as the most concrete draft of request and response shapes before implementation starts.
