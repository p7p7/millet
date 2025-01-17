use crate::parser::{ErrorKind, Expected, Parser};
use crate::top_dec::str_dec;
use sml_syntax::SyntaxKind as SK;

pub(crate) fn root(p: &mut Parser<'_>) {
  let entered = p.enter();
  while p.peek().is_some() {
    if !str_dec(p) {
      // avoid infinite loop
      p.error(ErrorKind::Expected(Expected::Item));
      p.bump();
    }
  }
  p.exit(entered, SK::Root);
}
