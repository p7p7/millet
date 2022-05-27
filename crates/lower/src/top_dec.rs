use crate::common::{get_name, get_path};
use crate::util::Cx;
use crate::{dec, ty};
use syntax::ast;

pub(crate) fn get(cx: &mut Cx, top_dec: ast::TopDec) -> hir::TopDec {
  match top_dec {
    ast::TopDec::StrDec(top_dec) => hir::TopDec::Str(get_str_dec(cx, Some(top_dec))),
    ast::TopDec::SigDec(top_dec) => hir::TopDec::Sig(
      top_dec
        .sig_binds()
        .filter_map(|sig_bind| {
          Some(hir::SigBind {
            name: get_name(sig_bind.name())?,
            sig_exp: get_sig_exp(cx, sig_bind.sig_exp()),
          })
        })
        .collect(),
    ),
    ast::TopDec::FunctorDec(top_dec) => hir::TopDec::Functor(
      top_dec
        .functor_binds()
        .filter_map(|fun_bind| {
          let functor_name = get_name(fun_bind.functor_name())?;
          let param_name = get_name(fun_bind.param())?;
          let mut body = get_str_exp(cx, fun_bind.body());
          if let Some(tail) = fun_bind.ascription_tail() {
            let (kind, sig_exp) = ascription_tail(cx, Some(tail));
            let asc = hir::StrExp::Ascription(body, kind, sig_exp);
            body = cx.str_exp(asc);
          }
          Some(hir::FunctorBind {
            functor_name,
            param_name,
            param_sig: get_sig_exp(cx, fun_bind.param_sig()),
            body,
          })
        })
        .collect(),
    ),
  }
}

fn get_str_dec(cx: &mut Cx, str_dec: Option<ast::StrDec>) -> hir::StrDecIdx {
  let mut str_decs: Vec<_> = str_dec?
    .str_dec_in_seqs()
    .map(|x| get_str_dec_one(cx, x.str_dec_one()?))
    .collect();
  if str_decs.len() == 1 {
    str_decs.pop().unwrap()
  } else {
    cx.str_dec(hir::StrDec::Seq(str_decs))
  }
}

fn get_str_dec_one(cx: &mut Cx, str_dec: ast::StrDecOne) -> hir::StrDecIdx {
  let res = match str_dec {
    ast::StrDecOne::DecStrDec(str_dec) => hir::StrDec::Dec(dec::get_one(cx, str_dec.dec_one()?)),
    ast::StrDecOne::StructureStrDec(str_dec) => hir::StrDec::Structure(
      str_dec
        .str_binds()
        .filter_map(|str_bind| {
          let mut str_exp = get_str_exp(cx, str_bind.str_exp());
          if let Some(tail) = str_bind.ascription_tail() {
            let (kind, sig_exp) = ascription_tail(cx, Some(tail));
            let asc = hir::StrExp::Ascription(str_exp, kind, sig_exp);
            str_exp = cx.str_exp(asc);
          }
          Some(hir::StrBind {
            name: get_name(str_bind.name())?,
            str_exp,
          })
        })
        .collect(),
    ),
    ast::StrDecOne::LocalStrDec(str_dec) => hir::StrDec::Local(
      get_str_dec(cx, str_dec.local_dec()),
      get_str_dec(cx, str_dec.in_dec()),
    ),
  };
  cx.str_dec(res)
}

fn get_str_exp(cx: &mut Cx, str_exp: Option<ast::StrExp>) -> hir::StrExpIdx {
  let ret = match str_exp? {
    ast::StrExp::StructStrExp(str_exp) => hir::StrExp::Struct(get_str_dec(cx, str_exp.str_dec())),
    ast::StrExp::PathStrExp(str_exp) => hir::StrExp::Path(get_path(str_exp.path()?)?),
    ast::StrExp::AscriptionStrExp(str_exp) => {
      let (kind, sig_exp) = ascription_tail(cx, str_exp.ascription_tail());
      hir::StrExp::Ascription(get_str_exp(cx, str_exp.str_exp()), kind, sig_exp)
    }
    ast::StrExp::AppStrExp(str_exp) => hir::StrExp::App(
      get_name(str_exp.name())?,
      get_str_exp(cx, str_exp.str_exp()),
    ),
    ast::StrExp::LetStrExp(str_exp) => hir::StrExp::Let(
      get_str_dec(cx, str_exp.str_dec()),
      get_str_exp(cx, str_exp.str_exp()),
    ),
  };
  cx.str_exp(ret)
}

fn get_sig_exp(cx: &mut Cx, sig_exp: Option<ast::SigExp>) -> hir::SigExpIdx {
  let ret = match sig_exp? {
    ast::SigExp::SigSigExp(sig_exp) => hir::SigExp::Spec(get_spec(cx, sig_exp.spec())),
    ast::SigExp::NameSigExp(sig_exp) => hir::SigExp::Name(get_name(sig_exp.name())?),
    ast::SigExp::WhereSigExp(sig_exp) => hir::SigExp::Where(
      get_sig_exp(cx, sig_exp.sig_exp()),
      ty::var_seq(sig_exp.ty_var_seq()),
      get_path(sig_exp.path()?)?,
      ty::get(cx, sig_exp.ty()),
    ),
  };
  cx.sig_exp(ret)
}

fn get_spec(cx: &mut Cx, spec: Option<ast::Spec>) -> hir::SpecIdx {
  let mut specs: Vec<_> = spec?
    .spec_in_seqs()
    .map(|x| get_spec_one(cx, x.spec_one()?))
    .collect();
  if specs.len() == 1 {
    specs.pop().unwrap()
  } else {
    cx.spec(hir::Spec::Seq(specs))
  }
}

fn get_spec_one(cx: &mut Cx, spec: ast::SpecOne) -> hir::SpecIdx {
  let ret = match spec {
    ast::SpecOne::ValSpec(spec) => hir::Spec::Val(
      spec
        .val_descs()
        .filter_map(|x| {
          Some(hir::ValDesc {
            name: get_name(x.name())?,
            ty: ty::get(cx, x.ty()),
          })
        })
        .collect(),
    ),
    ast::SpecOne::TySpec(spec) => hir::Spec::Ty(ty_descs(spec.ty_descs())),
    ast::SpecOne::EqTySpec(spec) => hir::Spec::EqTy(ty_descs(spec.ty_descs())),
    ast::SpecOne::DatSpec(spec) => hir::Spec::Datatype(dec::dat_binds(cx, spec.dat_binds())),
    ast::SpecOne::DatCopySpec(spec) => {
      hir::Spec::DatatypeCopy(get_name(spec.name())?, get_path(spec.path()?)?)
    }
    ast::SpecOne::ExSpec(spec) => hir::Spec::Exception(
      spec
        .ex_descs()
        .filter_map(|x| {
          Some(hir::ExDesc {
            name: get_name(x.name())?,
            ty: x.of_ty().map(|x| ty::get(cx, x.ty())),
          })
        })
        .collect(),
    ),
    ast::SpecOne::StrSpec(spec) => hir::Spec::Str(
      spec
        .str_descs()
        .filter_map(|x| {
          Some(hir::StrDesc {
            name: get_name(x.name())?,
            sig_exp: get_sig_exp(cx, x.sig_exp()),
          })
        })
        .collect(),
    ),
    ast::SpecOne::IncludeSpec(spec) => {
      let mut specs: Vec<_> = spec
        .sig_exps()
        .map(|x| hir::Spec::Include(get_sig_exp(cx, Some(x))))
        .collect();
      if specs.len() == 1 {
        specs.pop().unwrap()
      } else {
        hir::Spec::Seq(specs.into_iter().map(|x| cx.spec(x)).collect())
      }
    }
    ast::SpecOne::SharingSpec(_) => todo!(),
  };
  cx.spec(ret)
}

fn ascription_tail(
  cx: &mut Cx,
  tail: Option<ast::AscriptionTail>,
) -> (hir::Ascription, hir::SigExpIdx) {
  let kind = tail
    .as_ref()
    .and_then(|x| x.ascription())
    .map_or(hir::Ascription::Transparent, |x| match x.kind {
      ast::AscriptionKind::Colon => hir::Ascription::Transparent,
      ast::AscriptionKind::ColonGt => hir::Ascription::Opaque,
    });
  (kind, get_sig_exp(cx, tail.and_then(|x| x.sig_exp())))
}

fn ty_descs<I>(iter: I) -> Vec<hir::TyDesc>
where
  I: Iterator<Item = ast::TyDesc>,
{
  iter
    .filter_map(|x| {
      Some(hir::TyDesc {
        ty_vars: ty::var_seq(x.ty_var_seq()),
        name: get_name(x.name())?,
      })
    })
    .collect()
}
