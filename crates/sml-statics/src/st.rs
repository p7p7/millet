use crate::error::{Error, ErrorKind};
use crate::info::{Info, Mode};
use crate::pat_match::{Lang, Pat};
use crate::types::{Def, FixedTyVar, FixedTyVarGen, MetaTyVar, MetaTyVarGen, Subst, Syms, Ty};
use crate::util::apply;

/// The state.
///
/// Usually I call this `Cx` but the Definition defines a 'Context' already.
///
/// Invariant: 'Grows' monotonically.
#[derive(Debug)]
pub(crate) struct St {
  subst: Subst,
  errors: Vec<Error>,
  pub(crate) meta_gen: MetaTyVarGen,
  fixed_gen: FixedTyVarGen,
  info: Info,
  matches: Vec<Match>,
  holes: Vec<(MetaTyVar, sml_hir::Idx)>,
  pub(crate) syms: Syms,
}

impl St {
  pub(crate) fn new(mode: Mode, syms: Syms) -> Self {
    Self {
      subst: Subst::default(),
      errors: Vec::new(),
      meta_gen: MetaTyVarGen::default(),
      fixed_gen: FixedTyVarGen::default(),
      info: Info::new(mode),
      matches: Vec::new(),
      holes: Vec::new(),
      syms,
    }
  }

  pub(crate) fn mode(&self) -> &Mode {
    self.info.mode()
  }

  pub(crate) fn def(&self, idx: sml_hir::Idx) -> Option<Def> {
    self.mode().path().map(|path| Def { path, idx })
  }

  pub(crate) fn subst(&mut self) -> &mut Subst {
    &mut self.subst
  }

  pub(crate) fn err<I>(&mut self, idx: I, kind: ErrorKind)
  where
    I: Into<sml_hir::Idx>,
  {
    self.errors.push(Error {
      idx: idx.into(),
      kind,
    })
  }

  pub(crate) fn gen_fixed_var(&mut self, ty_var: sml_hir::TyVar) -> FixedTyVar {
    self.fixed_gen.gen(ty_var)
  }

  pub(crate) fn info(&mut self) -> &mut Info {
    &mut self.info
  }

  pub(crate) fn insert_bind(&mut self, pat: Pat, want: Ty, idx: sml_hir::Idx) {
    self.matches.push(Match {
      kind: MatchKind::Bind(pat),
      want,
      idx,
    })
  }

  pub(crate) fn insert_handle(&mut self, pats: Vec<Pat>, want: Ty, idx: sml_hir::Idx) {
    self.matches.push(Match {
      kind: MatchKind::Handle(pats),
      want,
      idx,
    })
  }

  pub(crate) fn insert_case(&mut self, pats: Vec<Pat>, want: Ty, idx: sml_hir::Idx) {
    self.matches.push(Match {
      kind: MatchKind::Case(pats),
      want,
      idx,
    })
  }

  pub(crate) fn insert_hole(&mut self, mv: MetaTyVar, idx: sml_hir::Idx) {
    self.holes.push((mv, idx));
  }

  pub(crate) fn finish(mut self) -> (Syms, Vec<Error>, Info) {
    let lang = Lang { syms: self.syms };
    let mut errors = self.errors;
    for (mv, idx) in self.holes {
      let mut ty = Ty::MetaVar(mv);
      apply(&self.subst, &mut ty);
      errors.push(Error {
        idx,
        kind: ErrorKind::ExpHole(ty),
      });
    }
    for mut m in self.matches {
      apply(&self.subst, &mut m.want);
      match m.kind {
        MatchKind::Bind(pat) => {
          let missing = get_match(&mut errors, &lang, vec![pat], m.want);
          if !missing.is_empty() {
            errors.push(Error {
              idx: m.idx,
              kind: ErrorKind::NonExhaustiveBinding(missing),
            });
          }
        }
        MatchKind::Case(pats) => {
          let missing = get_match(&mut errors, &lang, pats, m.want);
          if !missing.is_empty() {
            errors.push(Error {
              idx: m.idx,
              kind: ErrorKind::NonExhaustiveCase(missing),
            });
          }
        }
        MatchKind::Handle(pats) => {
          get_match(&mut errors, &lang, pats, m.want);
        }
      }
    }
    for ty in self.info.tys_mut() {
      apply(&self.subst, ty);
    }
    self.info.meta_vars = self.subst.into_meta_var_info();
    (lang.syms, errors, self.info)
  }
}

/// returns the missing pats
fn get_match(errors: &mut Vec<Error>, lang: &Lang, pats: Vec<Pat>, ty: Ty) -> Vec<Pat> {
  let ck = pattern_match::check(lang, pats, ty);
  let ck = match ck {
    Ok(x) => x,
    // we already should have emitted other errors in this case
    Err(_) => return Vec::new(),
  };
  let mut unreachable: Vec<_> = ck.unreachable.into_iter().flatten().collect();
  unreachable.sort_unstable_by_key(|x| x.into_raw());
  for idx in unreachable {
    errors.push(Error {
      idx: idx.into(),
      kind: ErrorKind::UnreachablePattern,
    });
  }
  ck.missing
}

#[derive(Debug)]
struct Match {
  kind: MatchKind,
  want: Ty,
  idx: sml_hir::Idx,
}

#[derive(Debug)]
enum MatchKind {
  Bind(Pat),
  Case(Vec<Pat>),
  Handle(Vec<Pat>),
}
