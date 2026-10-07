//! Heat's rules that talk to the outside world, with no network code of their
//! own: wi-core does the HTTP and hands the answers in (`docs/SPEC.md` 3).
//!
//! - [`ical`] reads the Brightspace calendar feed (RFC 5545).
//! - [`brightspace`] turns the feed into tasks: Heat's filters, the course and
//!   type, the duplicate test, the "No longer in Brightspace" tag, the sync
//!   rhythm and its status line (3.1, 3.11).
//! - [`mail`] is the school mail path: the Gmail query, pending grades and the
//!   announcements handed to Claude (3.1, 3.10).
//! - [`sync`] merges Heat records between devices and mi-wwav.com field by
//!   field (2.8, 9.7).
//! - [`assist`] is the client side of `/api/assist/:task`: consent, bodies,
//!   answers, failures and the one retry (2.11, 3.12).

pub mod assist;
pub mod brightspace;
pub mod ical;
pub mod mail;
pub mod sync;
