//! Agent modules for code fix.
//!
//! Each agent is a struct that encapsulates LLM configuration and provides
//! a `run()` method:
//!
//! - [`FixAgent`] – Main fix agent with exploration and file-reading tools.

pub mod fix;
pub mod prompt;

pub use fix::FixAgent;
pub use prompt::{
    build_confirmation_prompt, build_fix_prompt, build_exploration_prompt,
    build_verification_prompt, CONFIRM_SYSTEM_PROMPT, FIX_SYSTEM_PROMPT, VERIFY_SYSTEM_PROMPT,
};
