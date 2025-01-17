//! Process ML Basis files.
//!
//! From [the MLton docs](http://mlton.org/MLBasis).

#![deny(missing_debug_implementations, missing_docs, rust_2018_idioms)]

#[cfg(test)]
mod tests;

mod lex;
mod parse;
mod types;

pub use types::{BasDec, BasExp, Error, Namespace, ParsedPath, PathKind, Result};

/// Process the contents of a ML Basis file.
pub fn get(s: &str, env: &paths::slash_var_path::Env) -> Result<BasDec> {
  let tokens = lex::get(s)?;
  parse::get(&tokens, env)
}
