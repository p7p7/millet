//! Nested block comments delimited with `(*` and `*)`.

#![deny(missing_debug_implementations)]
#![deny(missing_docs)]
#![deny(rust_2018_idioms)]

use std::fmt;

/// A marker signifying a block comment was consumed.
#[derive(Debug)]
pub struct Consumed;

/// A kind of unmatched comment delimiter.
#[derive(Debug)]
pub enum Unmatched {
  /// Open comment, `(*`.
  Open,
  /// Close comment, `*)`.
  Close,
}

impl fmt::Display for Unmatched {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Unmatched::Open => f.write_str("open"),
      Unmatched::Close => f.write_str("close"),
    }
  }
}

/// Requires `bs.get(*idx) == Some(&b)`.
pub fn get(idx: &mut usize, b: u8, bs: &[u8]) -> Result<Option<Consumed>, Unmatched> {
  debug_assert_eq!(bs.get(*idx), Some(&b));
  if b == b'(' && bs.get(*idx + 1) == Some(&b'*') {
    *idx += 2;
    let mut level = 1_usize;
    loop {
      match (bs.get(*idx), bs.get(*idx + 1)) {
        (Some(&b'('), Some(&b'*')) => {
          *idx += 2;
          level += 1;
        }
        (Some(&b'*'), Some(&b')')) => {
          *idx += 2;
          level -= 1;
          if level == 0 {
            return Ok(Some(Consumed));
          }
        }
        (Some(_), Some(_)) => *idx += 1,
        (_, None) => return Err(Unmatched::Open),
        (None, Some(_)) => unreachable!(),
      }
    }
  }
  if b == b'*' && bs.get(*idx + 1) == Some(&b')') {
    *idx += 2;
    Err(Unmatched::Close)
  } else {
    Ok(None)
  }
}
