//! SQL workspace: statement policy, bounded execution, typed edits, transactions.
//! Everything here sits behind authentication, grants and an operator opt-in.

pub mod crud;
pub mod exec;
pub mod policy;
