//! Integration tests for `redkiln-core` that start no git process (TST-02). Those that do are in
//! the `repos` binary, and the subject cases in `subject`. `conformance` reads the SPEC's golden
//! fixtures and schemas (`spec/fixtures/`, `spec/schemas/`).

#[cfg(test)]
mod conformance;
#[cfg(test)]
mod envelope;
#[cfg(test)]
mod existing;
#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod import_mapped;
#[cfg(test)]
mod legacy;
#[cfg(test)]
mod migrate;
#[cfg(test)]
mod store;
#[cfg(test)]
mod support;
#[cfg(test)]
mod telemetry;
