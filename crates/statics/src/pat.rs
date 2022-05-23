use crate::error::Error;
use crate::pat_match::{Con, Pat};
use crate::st::St;
use crate::ty;
use crate::types::{Cx, IdStatus, Ty, ValEnv};
use crate::unify::unify;
use crate::util::{apply, get_env, get_scon, instantiate, record};

pub(crate) fn get(
  st: &mut St,
  cx: &Cx,
  ars: &hir::Arenas,
  ve: &mut ValEnv,
  pat: hir::PatIdx,
) -> (Pat, Ty) {
  match ars.pat[pat] {
    hir::Pat::None => (Pat::zero(Con::Any, pat), Ty::None),
    hir::Pat::Wild => any(st, pat),
    hir::Pat::SCon(ref scon) => {
      let con = match *scon {
        hir::SCon::Int(i) => Con::Int(i),
        hir::SCon::Real(_) => {
          st.err(Error::RealPat);
          Con::Any
        }
        hir::SCon::Word(w) => Con::Word(w),
        hir::SCon::Char(c) => Con::Char(c),
        hir::SCon::String(ref s) => Con::String(s.clone()),
      };
      (Pat::zero(con, pat), get_scon(scon))
    }
    hir::Pat::Con(ref path, arg) => {
      let is_var =
        arg.is_none() && path.structures().is_empty() && !cx.env.val_env.contains_key(path.last());
      if is_var {
        // TODO add to val env
        return any(st, pat);
      }
      let arg = arg.map(|x| get(st, cx, ars, ve, x));
      let env = match get_env(&cx.env, path) {
        Ok(x) => x,
        Err(_) => {
          st.err(Error::Undefined);
          return any(st, pat);
        }
      };
      let val_info = match env.val_env.get(path.last()) {
        Some(x) => x,
        None => {
          st.err(Error::Undefined);
          return any(st, pat);
        }
      };
      if let IdStatus::Val = val_info.id_status {
        st.err(Error::PatValIdStatus);
      }
      let ty = instantiate(st, &val_info.ty_scheme);
      let (sym, args, ty) = match ty {
        Ty::Con(_, sym) => {
          if arg.is_some() {
            st.err(Error::PatMustNotHaveArg)
          }
          (sym, Vec::new(), ty)
        }
        Ty::Fn(param_ty, mut res_ty) => {
          let sym = match res_ty.as_ref() {
            Ty::Con(_, x) => *x,
            _ => unreachable!(),
          };
          let arg_pat = match arg {
            None => {
              st.err(Error::PatMustHaveArg);
              Pat::zero(Con::Any, pat)
            }
            Some((arg_pat, arg_ty)) => {
              unify(st, *param_ty, arg_ty);
              apply(st.subst(), &mut res_ty);
              arg_pat
            }
          };
          (sym, vec![arg_pat], *res_ty)
        }
        _ => unreachable!(),
      };
      let pat = Pat::con(Con::Variant(sym, path.last().clone()), args, pat);
      (pat, ty)
    }
    hir::Pat::Record {
      ref rows,
      allows_other,
    } => {
      if allows_other {
        todo!()
      }
      let mut labs = Vec::<hir::Lab>::with_capacity(rows.len());
      let mut pats = Vec::<Pat>::with_capacity(rows.len());
      let ty = record(st, rows, |st, lab, pat| {
        let (pm_pat, ty) = get(st, cx, ars, ve, pat);
        labs.push(lab.clone());
        pats.push(pm_pat);
        ty
      });
      (Pat::con(Con::Record(labs), pats, pat), ty)
    }
    hir::Pat::Typed(pat, want) => {
      let (pm_pat, got) = get(st, cx, ars, ve, pat);
      let mut want = ty::get(st, cx, ars, want);
      unify(st, want.clone(), got);
      apply(st.subst(), &mut want);
      (pm_pat, want)
    }
    hir::Pat::As(_, pat) => {
      // TODO add name to val env
      get(st, cx, ars, ve, pat)
    }
  }
}

fn any(st: &mut St, pat: hir::PatIdx) -> (Pat, Ty) {
  (Pat::zero(Con::Any, pat), Ty::MetaVar(st.gen_meta_var()))
}
