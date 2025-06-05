use std::any::Any;

use crate::parser::Expr;

pub trait PrimitiveBinOps {
    fn as_any(&self) -> &dyn Any;

    #[allow(unused_variables)]
    fn bin_add(&self, other: &dyn PrimitiveBinOps) -> Option<Expr> {
        None
    }

    #[allow(unused_variables)]
    fn bin_sub(&self, other: &dyn PrimitiveBinOps) -> Option<Expr> {
        None
    }

    #[allow(unused_variables)]
    fn bin_mul(&self, other: &dyn PrimitiveBinOps) -> Option<Expr> {
        None
    }

    #[allow(unused_variables)]
    fn bin_div(&self, other: &dyn PrimitiveBinOps) -> Option<Expr> {
        None
    }
}
