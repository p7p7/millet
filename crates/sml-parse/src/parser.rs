//! A fork of event-parse from language-server-util, with SML-specific functionality.
//!
//! This includes:
//!
//! - Unbounded backtracking
//! - Dynamic operator precedence
//! - Custom errors (not just expected ..., found ...)
//!
//! Since it's not a library, we can also drop things like
//!
//! - the `Sink` trait
//! - the type parameter for the `SyntaxKind`
//! - the various bounds on the methods
//!
//! Feels a little bad to fork event-parse, but as I saw it, since the SML grammar has enough
//! oddities that it doesn't _quite_ fit, we'd either need to shove them into event-parse or fork.
//! And the library isn't even that big, so forking is... okay.

use drop_bomb::DropBomb;
use fast_hash::{map_with_capacity, FxHashMap};
use once_cell::sync::Lazy;
use sml_syntax::rowan::{GreenNodeBuilder, TextRange, TextSize};
use sml_syntax::token::{Token, Triviable};
use sml_syntax::{SyntaxKind as SK, SyntaxNode};
use std::fmt;

/// A mapping from names to (in)fixities.
pub type FixEnv = FxHashMap<str_util::Name, Infix>;

/// The default infix operators in the std basis.
pub static STD_BASIS: Lazy<FixEnv> = Lazy::new(|| {
  let ops_arr: [(Infix, &[&str]); 6] = [
    (Infix::left(7), &["*", "/", "div", "mod"]),
    (Infix::left(6), &["+", "-", "^"]),
    (Infix::right(5), &["::", "@"]),
    (Infix::left(4), &["=", "<>", ">", ">=", "<", "<="]),
    (Infix::left(3), &[":=", "o"]),
    (Infix::left(0), &["before"]),
  ];
  let mut ret = map_with_capacity(ops_arr.iter().map(|(_, names)| names.len()).sum());
  for (info, names) in ops_arr {
    for &name in names {
      ret.insert(str_util::Name::new(name), info);
    }
  }
  ret
});

/// A event-based parser for SML.
#[derive(Debug)]
pub(crate) struct Parser<'a> {
  tokens: &'a [Token<'a, SK>],
  tok_idx: usize,
  events: Vec<Option<Event>>,
  fix_env: &'a mut FixEnv,
}

impl<'a> Parser<'a> {
  /// Returns a new parser for the given tokens.
  pub(crate) fn new(tokens: &'a [Token<'a, SK>], fix_env: &'a mut FixEnv) -> Self {
    Self {
      tokens,
      tok_idx: 0,
      events: Vec::new(),
      fix_env,
    }
  }

  /// Starts parsing a syntax construct.
  ///
  /// The returned [`Entered`] must eventually be passed to [`Parser::exit`] or
  /// [`Parser::abandon`]. If it is not, it will panic when dropped.
  ///
  /// `Entered`s returned from `enter` should be consumed with `exit` or
  /// `abandon` in a FIFO manner. That is, the first most recently created
  /// `Entered` should be the first one to be consumed. (Might be more like
  /// first-out first-in in this case actually.)
  ///
  /// If this invariant isn't upheld, as in e.g.
  ///
  /// ```ignore
  /// let e1 = p.enter();
  /// let e2 = p.enter();
  /// p.exit(k, e1);
  /// ```
  ///
  /// then Weird Things might happen.
  pub(crate) fn enter(&mut self) -> Entered {
    let ev_idx = self.events.len();
    self.events.push(None);
    Entered {
      bomb: DropBomb::new("Entered markers must be exited"),
      ev_idx,
    }
  }

  /// Abandons parsing a syntax construct.
  ///
  /// The events recorded since this syntax construct began, if any, will belong
  /// to the parent.
  pub(crate) fn abandon(&mut self, mut en: Entered) {
    en.bomb.defuse();
    assert!(self.events[en.ev_idx].is_none());
  }

  /// Finishes parsing a syntax construct.
  pub(crate) fn exit(&mut self, mut en: Entered, kind: SK) -> Exited {
    en.bomb.defuse();
    let ev = &mut self.events[en.ev_idx];
    assert!(ev.is_none());
    *ev = Some(Event::Enter(kind, None));
    self.events.push(Some(Event::Exit));
    Exited { ev_idx: en.ev_idx }
  }

  /// Starts parsing a syntax construct and makes it the parent of the given
  /// completed node.
  ///
  /// Consider an expression grammar `<expr> ::= <int> | <expr> + <expr>`. When
  /// we see an `<int>`, we enter and exit an `<expr>` node for it. But then
  /// we see the `+` and realize the completed `<expr>` node for the int should
  /// be the child of a node for the `+`. That's when this function comes in.
  pub(crate) fn precede(&mut self, ex: Exited) -> Entered {
    let ret = self.enter();
    match self.events[ex.ev_idx] {
      Some(Event::Enter(_, ref mut parent)) => {
        assert!(parent.is_none());
        *parent = Some(ret.ev_idx);
      }
      ref ev => unreachable!("{:?} preceded {:?}, not Enter", ex, ev),
    }
    ret
  }

  /// Returns the token after the "current" token, or `None` if the parser is
  /// out of tokens.
  ///
  /// Equivalent to `self.peek_n(0)`. See [`Parser::peek_n`].
  pub(crate) fn peek(&mut self) -> Option<Token<'a, SK>> {
    while let Some(&tok) = self.tokens.get(self.tok_idx) {
      if tok.kind.is_trivia() {
        self.tok_idx += 1;
      } else {
        return Some(tok);
      }
    }
    None
  }

  /// Returns the token `n` tokens in front of the current token, or `None` if
  /// there is no such token.
  ///
  /// The current token is the first token not yet consumed for which
  /// [`Triviable::is_trivia`] returns `true`; thus, if this returns
  /// `Some(tok)`, then `tok.kind.is_trivia()` is `false`.
  pub(crate) fn peek_n(&mut self, n: usize) -> Option<Token<'a, SK>> {
    let mut ret = self.peek();
    let old_tok_idx = self.tok_idx;
    for _ in 0..n {
      self.tok_idx += 1;
      ret = self.peek();
    }
    self.tok_idx = old_tok_idx;
    ret
  }

  /// Consumes and returns the current token.
  ///
  /// Panics if there are no more tokens, i.e. if [`Parser::peek`] would return
  /// `None` just prior to calling this.
  ///
  /// This is often used after calling [`Parser::at`] to verify some expected
  /// token was present.
  pub(crate) fn bump(&mut self) -> Token<'a, SK> {
    let ret = self.peek().expect("bump with no tokens");
    self.events.push(Some(Event::Token));
    self.tok_idx += 1;
    ret
  }

  /// Records an error at the current token.
  pub(crate) fn error(&mut self, kind: ErrorKind) {
    self.events.push(Some(Event::Error(kind)));
  }

  fn eat_trivia(&mut self, sink: &mut BuilderSink) {
    while let Some(&tok) = self.tokens.get(self.tok_idx) {
      if !tok.kind.is_trivia() {
        break;
      }
      sink.token(tok);
      self.tok_idx += 1;
    }
  }

  /// Finishes parsing, and writes the parsed tree into the `sink`.
  pub(crate) fn finish(mut self) -> (SyntaxNode, Vec<Error>) {
    let mut sink = BuilderSink::default();
    self.tok_idx = 0;
    let mut kinds = Vec::new();
    let mut levels: usize = 0;
    for idx in 0..self.events.len() {
      let ev = match self.events[idx].take() {
        Some(ev) => ev,
        None => continue,
      };
      match ev {
        Event::Enter(kind, mut parent) => {
          assert!(kinds.is_empty());
          kinds.push(kind);
          while let Some(p) = parent {
            match self.events[p].take() {
              Some(Event::Enter(kind, new_parent)) => {
                kinds.push(kind);
                parent = new_parent;
              }
              // abandoned precede
              None => break,
              ev => unreachable!("{:?} was {:?}, not Enter", parent, ev),
            }
          }
          for kind in kinds.drain(..).rev() {
            // keep as much trivia as possible outside of what we're entering.
            if levels != 0 {
              self.eat_trivia(&mut sink);
            }
            sink.enter(kind);
            levels += 1;
          }
        }
        Event::Exit => {
          sink.exit();
          levels -= 1;
          // keep as much trivia as possible outside of top-level items.
          if levels == 1 {
            self.eat_trivia(&mut sink);
          }
        }
        Event::Token => {
          self.eat_trivia(&mut sink);
          sink.token(self.tokens[self.tok_idx]);
          self.tok_idx += 1;
        }
        Event::Error(kind) => sink.error(kind),
      }
    }
    assert_eq!(levels, 0);
    sink.extend_errors();
    (SyntaxNode::new_root(sink.builder.finish()), sink.errors)
  }

  /// Returns whether the current token has the given `kind`.
  pub(crate) fn at(&mut self, kind: SK) -> bool {
    self.at_n(0, kind)
  }

  /// Returns whether the token `n` ahead has the given `kind`.
  pub(crate) fn at_n(&mut self, n: usize, kind: SK) -> bool {
    self.peek_n(n).map_or(false, |tok| tok.kind == kind)
  }

  /// If the current token's kind is `kind`, then this consumes it, else this
  /// errors. Returns the token if it was eaten.
  pub(crate) fn eat(&mut self, kind: SK) -> Option<Token<'a, SK>> {
    if self.at(kind) {
      Some(self.bump())
    } else {
      self.error(ErrorKind::Expected(Expected::Kind(kind)));
      None
    }
  }

  // sml-specific methods //

  pub(crate) fn insert_infix(&mut self, name: &str, info: Infix) {
    self.fix_env.insert(str_util::Name::new(name), info);
  }

  pub(crate) fn get_infix(&mut self, name: &str) -> Option<Infix> {
    self.fix_env.get(name).copied()
  }

  pub(crate) fn is_infix(&mut self, name: &str) -> bool {
    self.fix_env.contains_key(name)
  }

  pub(crate) fn remove_infix(&mut self, name: &str) {
    self.fix_env.remove(name);
  }

  /// Save the state of the parser.
  ///
  /// Use it with `ok_since` to implement unbounded backtracking.
  ///
  /// For any `Entered` or `Exited` that were created before the save, do not `exit` or `precede`
  /// them respectively between the save and the `ok_since`. Or do anything else that modifies
  /// any events before the save. Otherwise the parser won't fully recover to its original state
  /// before the save.
  ///
  /// It's intended to be used like this:
  ///
  /// ```ignore
  /// let save = p.save();
  /// // maybe make some new `Entered` and `Exited`
  /// // maybe eat some tokens
  /// // maybe encounter some errors
  /// foo(p);
  /// if p.ok_since(save) {
  ///   // foo parsed without errors
  /// } else {
  ///   // foo had errors, so it failed to parse.
  ///   // the parser is reset to the state at the save
  /// }
  /// ```
  pub(crate) fn save(&self) -> Save {
    Save {
      tok_idx: self.tok_idx,
      events_len: self.events.len(),
    }
  }

  /// returns whether the save was discarded (i.e. did NOT restore to that save)
  pub(crate) fn ok_since(&mut self, save: Save) -> bool {
    let error_since = self
      .events
      .iter()
      .skip(save.events_len)
      .any(|ev| matches!(*ev, Some(Event::Error(..))));
    if error_since {
      self.tok_idx = save.tok_idx;
      self.events.truncate(save.events_len);
    }
    !error_since
  }
}

/// A marker for a syntax construct that is mid-parse. If this is not consumed
/// by a [`Parser`], it will panic when dropped.
#[derive(Debug)]
pub(crate) struct Entered {
  bomb: DropBomb,
  ev_idx: usize,
}

/// A marker for a syntax construct that has been fully parsed.
///
/// We let this be `Copy` so we can do things like this:
/// ```ignore
/// let mut ex: Exited = ...;
/// loop {
///   let en = p.precede(ex);
///   if ... {
///     ...;
///     ex = p.exit(en, ...);
///   } else {
///     p.abandon(en);
///     return Some(ex);
///   }
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub(crate) struct Exited {
  ev_idx: usize,
}

enum Event {
  Enter(SK, Option<usize>),
  Token,
  Exit,
  Error(ErrorKind),
}

impl fmt::Debug for Event {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Event::Enter(_, n) => f.debug_tuple("Enter").field(n).finish(),
      Event::Token => f.debug_tuple("Token").finish(),
      Event::Exit => f.debug_tuple("Exit").finish(),
      Event::Error(k) => f.debug_tuple("Error").field(k).finish(),
    }
  }
}

// sml-specific types //

/// Information about an infix name.
#[derive(Debug, Clone, Copy)]
pub struct Infix {
  /// The precedence.
  pub prec: u16,
  /// The associativity.
  pub assoc: Assoc,
}

impl Infix {
  /// Returns a new Infix with left associativity.
  pub(crate) fn left(prec: u16) -> Self {
    Self {
      prec,
      assoc: Assoc::Left,
    }
  }

  /// Returns a new Infix with right associativity.
  pub(crate) fn right(prec: u16) -> Self {
    Self {
      prec,
      assoc: Assoc::Right,
    }
  }
}

/// Associativity for infix operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Assoc {
  Left,
  Right,
}

#[derive(Debug)]
pub(crate) struct Save {
  tok_idx: usize,
  events_len: usize,
}

/// A parse error.
#[derive(Debug)]
pub struct Error {
  range: TextRange,
  kind: ErrorKind,
}

impl Error {
  /// Returns the range for this.
  pub fn range(&self) -> TextRange {
    self.range
  }

  /// Returns a value that displays the message.
  pub fn display(&self) -> impl fmt::Display + '_ {
    &self.kind
  }

  /// Returns the code for this.
  pub fn to_code(&self) -> u16 {
    match self.kind {
      ErrorKind::NotInfix => 3001,
      ErrorKind::InfixWithoutOp => 3002,
      ErrorKind::InvalidFixity(_) => 3003,
      ErrorKind::NegativeFixity => 3004,
      ErrorKind::SameFixityDiffAssoc => 3005,
      ErrorKind::Expected(_) => 3006,
    }
  }
}

#[derive(Debug)]
pub(crate) enum ErrorKind {
  NotInfix,
  InfixWithoutOp,
  InvalidFixity(std::num::ParseIntError),
  NegativeFixity,
  SameFixityDiffAssoc,
  Expected(Expected),
}

impl fmt::Display for ErrorKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ErrorKind::NotInfix => f.write_str("non-infix name used as infix"),
      ErrorKind::InfixWithoutOp => f.write_str("infix name used as non-infix without `op`"),
      ErrorKind::InvalidFixity(e) => write!(f, "invalid fixity: {e}"),
      ErrorKind::NegativeFixity => f.write_str("fixity is negative"),
      ErrorKind::SameFixityDiffAssoc => {
        f.write_str("consecutive infix names with same fixity but different associativity")
      }
      ErrorKind::Expected(e) => write!(f, "expected {e}"),
    }
  }
}

#[derive(Debug)]
pub(crate) enum Expected {
  Exp,
  Lab,
  Pat,
  Path,
  SigExp,
  StrExp,
  Ty,
  LRoundExpTail,
  Item,
  Kind(SK),
}

impl fmt::Display for Expected {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Expected::Exp => f.write_str("an expression"),
      Expected::Lab => f.write_str("a label"),
      Expected::Pat => f.write_str("a pattern"),
      Expected::Path => f.write_str("a path"),
      Expected::SigExp => f.write_str("a signature expression"),
      Expected::StrExp => f.write_str("a structure expression"),
      Expected::Ty => f.write_str("a type"),
      Expected::LRoundExpTail => f.write_str("`)`, `,`, or `;`"),
      Expected::Item => f.write_str("a top-level item"),
      Expected::Kind(k) => k.fmt(f),
    }
  }
}

#[derive(Default)]
struct BuilderSink {
  builder: GreenNodeBuilder<'static>,
  range: TextRange,
  errors: Vec<Error>,
  kinds: Vec<ErrorKind>,
}

impl BuilderSink {
  fn extend_errors(&mut self) {
    let errors = std::mem::take(&mut self.kinds)
      .into_iter()
      .map(|kind| Error {
        range: self.range,
        kind,
      });
    self.errors.extend(errors);
  }

  fn enter(&mut self, kind: SK) {
    self.builder.start_node(kind.into());
  }

  fn token(&mut self, token: Token<'_, SK>) {
    let is_trivia = token.kind.is_trivia();
    self.builder.token(token.kind.into(), token.text);
    let start = self.range.end();
    let end = start + TextSize::of(token.text);
    self.range = TextRange::new(start, end);
    if !is_trivia {
      self.extend_errors();
    }
  }

  fn exit(&mut self) {
    self.builder.finish_node();
  }

  fn error(&mut self, kind: ErrorKind) {
    self.kinds.push(kind);
  }
}
