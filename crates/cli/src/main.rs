//! A CLI for millet.

mod args;
mod diagnostic;
mod source;

use codespan_reporting::term;
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};
use millet_core::{error, lex, parse};
use std::io::Write as _;

fn run() -> bool {
  let args = args::get();
  let config = term::Config::default();
  let w = StandardStream::stdout(ColorChoice::Auto);
  let mut w = w.lock();
  let mut m = source::SourceMap::new();
  for name in args.files {
    match std::fs::read_to_string(&name) {
      Ok(s) => m.insert(name, s),
      Err(e) => {
        writeln!(w, "io error: {}: {}", name, e).unwrap();
        return false;
      }
    }
  }
  for (id, file) in m.iter() {
    let lexer = match lex::get(file.as_bytes()) {
      Ok(x) => x,
      Err(e) => {
        term::emit(
          &mut w,
          &config,
          &m,
          &diagnostic::new(id, e.loc.wrap(error::Error::Lex(e.val))),
        )
        .unwrap();
        return false;
      }
    };
    match parse::get(lexer) {
      Ok(xs) => eprintln!("parsed: {:#?}", xs),
      Err(e) => {
        term::emit(
          &mut w,
          &config,
          &m,
          &diagnostic::new(id, e.loc.wrap(error::Error::Parse(e.val))),
        )
        .unwrap();
        return false;
      }
    }
  }
  true
}

fn main() {
  if !run() {
    std::process::exit(1);
  }
  println!("OK");
}
