//! Config-slot type system (,): typed slot declarations that structurally
//! cannot carry a secret/identity value, classification into identity-bearing vs
//! behavior-relevant, aggregation across a component graph, and deviation detection for
//! behavior-relevant slots.
//!
//! See `types`/`classification` module docs for the structural "secrets never travel"
//! guarantee this module tree enforces, and `aggregate`/`deviation` for the pure functions
//! the pre-flight will call.
//!
//! `required` answers the question `deviation` structurally cannot: which slots must the
//! target environment fill? `detect_deviation` returns `None` for every identity-bearing
//! slot by design, so it can never name a credential; `required_slots` returns both families.

pub mod aggregate;
pub mod classification;
pub mod deviation;
pub mod required;
pub mod types;

pub use aggregate::aggregate;
pub use classification::{classify, SlotClass};
pub use deviation::{detect_deviation, Deviation};
pub use required::{classify_slots, required_slots, ClassifiedSlot, RequiredSlot};
pub use types::{ConfigSlot, SlotType, SuppliedBy};
