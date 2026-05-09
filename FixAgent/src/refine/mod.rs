//! Patch validation and refinement for fix quality.
//!
//! This module refines raw LLM patch output by validating patch correctness
//! and applying safe corrections.

pub mod patch_validator;

pub use patch_validator::PatchValidator;
