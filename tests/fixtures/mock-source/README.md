# Core WASM test fixture

This synthetic component implements `wit/memcastle-source.wit` for Core lifecycle tests.
It returns two fixed candidates, advances a JSON cursor and uses the locator as searchable document text.
For multi-module plugin tests, locators beginning `first:` or `second:` select that source identity without building a second guest.
Its filesystem permission exists to test consent; the component does not read files or contact providers.

`tests/common/wasm.rs` builds it in the shared debug target directory for the generic WASM suite.
Provider behavior and conformance belong to each source's own test leg, not this fixture.
