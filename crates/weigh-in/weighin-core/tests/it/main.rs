//! Property tests for `weighin-core`, compiled as one binary. Each property drives the public API
//! with a few thousand generated cases from a fixed-seed PRNG in `support`, so runs are
//! reproducible without a property-testing dependency.

#[cfg(test)]
mod atom_props;
#[cfg(test)]
mod id_props;
#[cfg(test)]
mod marker_props;
#[cfg(test)]
mod projection_props;
#[cfg(test)]
mod review_props;
#[cfg(test)]
mod support;
