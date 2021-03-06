use super::*;
use ast::{Kind, Row, Type, TypeKind, Variant};

pub trait TypeVisitor<'t>: Sized {
    fn visit_defined(&mut self, _: &'t str) {}

    fn visit_variable(&mut self, _: &'t str) {}

    fn visit_function(&mut self, ty1: &'t Type, ty2: &'t Type) {
        self.visit_ty(ty1);
        self.visit_ty(ty2);
    }

    fn visit_application(&mut self, ty1: &'t Type, ty2: &'t Type) {
        self.visit_ty(ty1);
        self.visit_ty(ty2);
    }

    fn visit_sum(&mut self, var: &'t [Variant]) {
        for v in var {
            if let Some(ty) = &v.ty {
                self.visit_ty(ty);
            }
        }
    }

    fn visit_product(&mut self, var: &'t [Type]) {
        for v in var {
            self.visit_ty(v);
        }
    }

    fn visit_record(&mut self, var: &'t [Row]) {
        for v in var {
            self.visit_ty(&v.ty);
        }
    }

    fn visit_existential(&mut self, _: &'t str, _: &'t Kind, ty: &'t Type) {
        self.visit_ty(ty);
    }

    fn visit_universal(&mut self, _: &'t str, _: &'t Kind, ty: &'t Type) {
        self.visit_ty(ty);
    }

    fn visit_abstraction(&mut self, _: &'t str, _: &'t Kind, ty: &'t Type) {
        self.visit_ty(ty);
    }

    fn visit_recursive(&mut self, ty: &'t Type) {
        self.visit_ty(ty);
    }

    fn visit_ty(&mut self, ty: &'t Type) {
        self.walk_ty(ty);
    }

    fn walk_ty(&mut self, ty: &'t Type) {
        use TypeKind::*;
        match &ty.kind {
            Int => {}
            Bool => {}
            Unit => {}
            Infer => {}
            Defined(s) => self.visit_defined(s),
            Variable(s) => self.visit_variable(s),
            Function(ty1, ty2) => self.visit_function(ty1, ty2),
            Sum(var) => self.visit_sum(var),
            Product(tys) => self.visit_product(tys),
            Record(rows) => self.visit_record(rows),
            Existential(s, k, ty) => self.visit_existential(s, k, ty),
            Universal(s, k, ty) => self.visit_universal(s, k, ty),
            Abstraction(s, k, ty) => self.visit_abstraction(s, k, ty),
            Application(ty1, ty2) => self.visit_application(ty1, ty2),
            Recursive(ty) => self.visit_recursive(ty),
        }
    }
}
