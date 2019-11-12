//! Typechecking of the simply typed lambda calculus with parametric
//! polymorphism
pub mod patterns;
pub mod visit;
use crate::terms::{Kind, Literal, Pattern, Primitive, Term};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use util::span::Span;
use visit::{MutVisitor, Shift, Subst};

#[derive(Clone, PartialEq, PartialOrd, Eq, Hash)]
pub enum Type {
    Unit,
    Nat,
    Bool,
    Alias(String),
    Var(usize),
    Variant(Vec<Variant>),
    Product(Vec<Type>),
    Arrow(Box<Type>, Box<Type>),
    Universal(Box<Type>),
    Rec(Box<Type>),
}

#[derive(Clone, PartialEq, PartialOrd, Eq, Hash)]
pub struct Variant {
    pub label: String,
    pub ty: Type,
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct TypeError {
    pub span: Span,
    pub kind: TypeErrorKind,
}

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub enum TypeErrorKind {
    ParameterMismatch(Box<Type>, Box<Type>, Span),

    InvalidProjection,
    NotArrow,
    NotUniversal,
    NotVariant,
    NotProduct,
    NotRec,
    IncompatibleArms,
    InvalidPattern,
    NotExhaustive,
    UnreachablePattern,
    UnboundVariable(usize),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Context {
    stack: VecDeque<Type>,
    map: HashMap<String, Type>,
}

impl Context {
    fn push(&mut self, ty: Type) {
        self.stack.push_front(ty);
    }

    fn pop(&mut self) {
        self.stack
            .pop_front()
            .expect("Context::pop() with empty type stack");
    }

    fn find(&self, idx: usize) -> Option<&Type> {
        self.stack.get(idx)
    }

    pub fn alias(&mut self, alias: String, ty: Type) {
        self.map.insert(alias, ty);
    }

    fn aliaser(&self) -> Aliaser<'_> {
        Aliaser { map: &self.map }
    }

    pub fn de_alias(&mut self, term: &mut Term) {
        crate::terms::visit::MutVisitor::visit(self, term)
    }
}

/// Helper function for extracting type from a variant
fn variant_field<'vs>(
    var: &'vs [Variant],
    label: &str,
    span: Span,
) -> Result<&'vs Type, TypeError> {
    for f in var {
        if label == f.label {
            return Ok(&f.ty);
        }
    }
    Err(TypeError {
        span,
        kind: TypeErrorKind::NotVariant,
    })
}

impl Context {
    /// Return an error with a span derived from `term` and `kind`
    pub const fn error(term: &Term, kind: TypeErrorKind) -> Result<Type, TypeError> {
        Err(TypeError {
            span: term.span,
            kind,
        })
    }

    pub fn type_check(&mut self, term: &Term) -> Result<Type, TypeError> {
        // dbg!(&self.stack);
        // println!("{}", term);
        match term.kind() {
            Kind::Lit(Literal::Unit) => Ok(Type::Unit),
            Kind::Lit(Literal::Bool(_)) => Ok(Type::Bool),
            Kind::Lit(Literal::Nat(_)) => Ok(Type::Nat),
            Kind::Var(idx) => self.find(*idx).cloned().ok_or_else(|| TypeError {
                span: term.span,
                kind: TypeErrorKind::UnboundVariable(*idx),
            }),
            Kind::Abs(ty, t2) => {
                self.push(*ty.clone());
                let ty2 = self.type_check(t2)?;
                // Shift::new(-1).visit(&mut ty2);
                self.pop();
                Ok(Type::Arrow(ty.clone(), Box::new(ty2)))
            }
            Kind::App(t1, t2) => {
                let ty1 = self.type_check(t1)?;
                let ty2 = self.type_check(t2)?;
                match ty1 {
                    Type::Arrow(ty11, ty12) => {
                        if *ty11 == ty2 {
                            Ok(*ty12)
                        } else {
                            Context::error(
                                t1,
                                TypeErrorKind::ParameterMismatch(ty11, Box::new(ty2), t2.span),
                            )
                        }
                    }
                    _ => Context::error(term, TypeErrorKind::NotArrow),
                }
            }
            Kind::Fix(inner) => {
                let ty = self.type_check(inner)?;
                match ty {
                    Type::Arrow(ty1, ty2) => {
                        if ty1 == ty2 {
                            Ok(*ty1)
                        } else {
                            Context::error(
                                term,
                                TypeErrorKind::ParameterMismatch(ty1, ty2, inner.span),
                            )
                        }
                    }
                    _ => Context::error(term, TypeErrorKind::NotArrow),
                }
            }
            Kind::Primitive(prim) => match prim {
                Primitive::IsZero => Ok(Type::Arrow(Box::new(Type::Nat), Box::new(Type::Bool))),
                _ => Ok(Type::Arrow(Box::new(Type::Nat), Box::new(Type::Nat))),
            },
            Kind::Injection(label, tm, ty) => match ty.as_ref() {
                Type::Variant(fields) => {
                    for f in fields {
                        if label == &f.label {
                            let ty_ = self.type_check(tm)?;
                            if ty_ == f.ty {
                                return Ok(*ty.clone());
                            } else {
                                return Context::error(
                                    term,
                                    TypeErrorKind::ParameterMismatch(
                                        Box::new(ty_),
                                        Box::new(f.ty.clone()),
                                        tm.span,
                                    ),
                                );
                            }
                        }
                    }
                    Context::error(term, TypeErrorKind::NotVariant)
                }
                _ => Context::error(term, TypeErrorKind::NotVariant),
            },
            Kind::Projection(term, idx) => match self.type_check(term)? {
                Type::Product(types) => match types.get(*idx) {
                    Some(ty) => Ok(ty.clone()),
                    None => Context::error(term, TypeErrorKind::InvalidProjection),
                },
                _ => Context::error(term, TypeErrorKind::NotProduct),
            },
            Kind::Product(terms) => Ok(Type::Product(
                terms
                    .iter()
                    .map(|t| self.type_check(t))
                    .collect::<Result<_, _>>()?,
            )),
            Kind::Let(t1, t2) => {
                let ty = self.type_check(t1)?;
                self.push(ty);
                let y = self.type_check(t2);
                self.pop();
                y
            }
            Kind::TyAbs(term) => {
                let ty2 = self.type_check(term)?;
                Ok(Type::Universal(Box::new(ty2)))
            }
            Kind::TyApp(term, ty) => {
                let mut ty = ty.clone();
                let ty1 = self.type_check(term)?;
                match ty1 {
                    Type::Universal(mut ty12) => {
                        Shift::new(1).visit(&mut ty);
                        Subst::new(*ty).visit(&mut ty12);
                        Shift::new(-1).visit(&mut ty12);
                        Ok(*ty12)
                    }
                    _ => {
                        dbg!(ty1);
                        Context::error(term, TypeErrorKind::NotUniversal)
                    }
                }
            }
            // See src/types/patterns.rs for exhaustiveness and typechecking
            // of case expressions
            Kind::Case(expr, arms) => self.type_check_case(expr, arms),

            Kind::Unfold(rec, tm) => match rec.as_ref() {
                Type::Rec(inner) => {
                    let s = subst(*rec.clone(), *inner.clone());
                    // Ok(Type::Arrow(rec.clone(), Box::new(s)))
                    Ok(s)
                }
                _ => Context::error(term, TypeErrorKind::NotRec),
            },

            Kind::Fold(rec, tm) => match rec.as_ref() {
                Type::Rec(inner) => {
                    // let s = subst(*rec.clone(), *inner.clone());

                    // Ok(Type::Arrow(Box::new(s), rec.clone()))

                    Ok(*rec.clone())
                }
                _ => {
                    dbg!(rec);
                    Context::error(term, TypeErrorKind::NotRec)
                }
            },
        }
    }
}

pub fn subst(mut s: Type, mut t: Type) -> Type {
    Shift::new(1).visit(&mut s);
    Subst::new(s).visit(&mut t);
    Shift::new(-1).visit(&mut t);
    t
}

struct Aliaser<'ctx> {
    map: &'ctx HashMap<String, Type>,
}

impl<'ctx> MutVisitor for Aliaser<'ctx> {
    fn visit(&mut self, ty: &mut Type) {
        match ty {
            Type::Unit | Type::Bool | Type::Nat => {}
            Type::Var(v) => {}
            Type::Alias(v) => {
                if let Some(aliased) = self.map.get(v) {
                    *ty = aliased.clone();
                }
            }
            Type::Variant(v) => self.visit_variant(v),
            Type::Product(v) => self.visit_product(v),

            Type::Arrow(ty1, ty2) => self.visit_arrow(ty1, ty2),
            Type::Universal(ty) => self.visit_universal(ty),
            Type::Rec(ty) => self.visit_rec(ty),
        }
    }
}

impl crate::terms::visit::MutVisitor for Context {
    fn visit_abs(&mut self, sp: &mut Span, ty: &mut Type, term: &mut Term) {
        self.aliaser().visit(ty);
        self.visit(term);
    }

    fn visit_tyapp(&mut self, sp: &mut Span, term: &mut Term, ty: &mut Type) {
        self.aliaser().visit(ty);
        self.visit(term);
    }

    fn visit_injection(
        &mut self,
        sp: &mut Span,
        label: &mut String,
        term: &mut Term,
        ty: &mut Type,
    ) {
        self.aliaser().visit(ty);
        self.visit(term);
    }

    fn visit_fold(&mut self, sp: &mut Span, ty: &mut Type, tm: &mut Term) {
        self.aliaser().visit(ty);
        self.visit(tm);
    }

    fn visit_unfold(&mut self, sp: &mut Span, ty: &mut Type, tm: &mut Term) {
        self.aliaser().visit(ty);
        self.visit(tm);
    }
}

impl fmt::Debug for Type {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Type::Unit => write!(f, "Unit"),
            Type::Bool => write!(f, "Bool"),
            Type::Nat => write!(f, "Nat"),
            Type::Var(v) => write!(f, "TyVar({})", v),
            Type::Variant(v) => write!(
                f,
                "{:?}",
                v.iter()
                    .map(|x| format!("{}: {:?}", x.label, x.ty))
                    .collect::<Vec<String>>()
                    .join(" | ")
            ),
            Type::Product(v) => write!(
                f,
                "({})",
                v.iter()
                    .map(|x| format!("{:?}", x))
                    .collect::<Vec<String>>()
                    .join(",")
            ),
            Type::Alias(s) => write!(f, "{}", s),
            Type::Arrow(t1, t2) => write!(f, "({:?}->{:?})", t1, t2),
            Type::Universal(ty) => write!(f, "forall X.{:?}", ty),
            Type::Rec(ty) => write!(f, "rec {:?}", ty),
        }
    }
}
