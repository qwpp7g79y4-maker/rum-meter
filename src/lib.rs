//! Estimate billable HCP Terraform RUM from local state files.
//!
//! [`state`] reads Terraform state format version 4. [`rum`] applies rules
//! R1-R4. [`diff`] compares two classified states. [`report`], [`render`] and
//! [`doc`] build the text, JSON, Markdown and HTML reports. [`cost`] prices a
//! count from a file the caller supplies. [`encoding`] refuses UTF-16 and
//! UTF-32 before those parsers run.

pub mod cost;
pub mod diff;
pub mod doc;
pub mod encoding;
mod regular;
pub mod render;
pub mod report;
pub mod rum;
pub mod state;
