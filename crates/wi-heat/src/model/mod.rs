//! Heat's maths, ported from `app/ui/src/heat/model` so the Rust core and the
//! MCP helper compute exactly what the app does (`docs/SPEC.md` chapter 3).
//!
//! Plain functions over plain structs. Time comes in as a parameter (`now`, in
//! epoch ms) and a zone (`&jiff::tz::TimeZone`); nothing here reads the clock,
//! does I/O or holds state. A function that makes records takes the next id
//! from a `&mut dyn FnMut() -> Id`.
//!
//! The port gives the TypeScript's answer for every input, down to its sums:
//! numbers are `f64` as they are there, and [`js`] holds the JavaScript the
//! TypeScript leans on (`Math.round`'s ties, `String(n)`, `Date.UTC`,
//! `localeCompare`, a stable sort). `tests/model_vectors.rs` replays inputs
//! and outputs the TypeScript wrote and checks them bit for bit.
//!
//! One file per TypeScript module: [`records`], [`heat`], [`estimate`],
//! [`grades`], [`plan`], [`focus`], [`habits`], [`recurrence`], [`calendar`],
//! [`review`], [`spaces`], [`import_artifact`], and [`lcd`] for the Now
//! strip's task half. [`zone`] and [`format`] are `shared/time` (the zone
//! maths and the clock), and [`copy`] holds the sentences the functions here
//! build. Left in TypeScript, because it is only labels, colours and layout:
//! the tab bar, the rest of `copy.ts`, the level colours and tube, the letter
//! pill's colour, and the titles and headers that name a view.

pub mod calendar;
pub mod copy;
pub mod estimate;
pub mod focus;
pub mod format;
pub mod grades;
pub mod habits;
pub mod heat;
pub mod js;
pub mod lcd;
pub mod plan;
pub mod records;
pub mod recurrence;
pub mod review;
pub mod spaces;
pub mod zone;
