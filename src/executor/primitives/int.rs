use crate::{
    executor::primitives::{
        FloatPrimitive,
        traits::{PrimitiveValue, bin_ops::PrimitiveBinOps},
    },
    parser::Expr,
};

#[derive(Debug)]
pub struct IntPrimitive {
    pub value: i64,
}

impl PrimitiveValue<i64> for IntPrimitive {
    fn display(&self) -> String {
        self.value.to_string()
    }

    fn from_value_to_expr(value: i64) -> Expr {
        Expr::Int(Self { value })
    }
}

impl PrimitiveBinOps for IntPrimitive {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn bin_add(&self, other: &dyn PrimitiveBinOps) -> Option<Expr> {
        if let Some(other) = other.as_any().downcast_ref::<IntPrimitive>() {
            Some(IntPrimitive::from_value_to_expr(self.value + other.value))
        } else {
            None
        }
    }

    fn bin_sub(&self, other: &dyn PrimitiveBinOps) -> Option<Expr> {
        if let Some(other) = other.as_any().downcast_ref::<IntPrimitive>() {
            Some(IntPrimitive::from_value_to_expr(self.value - other.value))
        } else {
            None
        }
    }

    fn bin_mul(&self, other: &dyn PrimitiveBinOps) -> Option<Expr> {
        if let Some(other) = other.as_any().downcast_ref::<IntPrimitive>() {
            Some(IntPrimitive::from_value_to_expr(self.value * other.value))
        } else {
            None
        }
    }

    fn bin_div(&self, other: &dyn PrimitiveBinOps) -> Option<Expr> {
        if let Some(other) = other.as_any().downcast_ref::<IntPrimitive>() {
            Some(FloatPrimitive::from_value_to_expr(
                self.value as f64 / other.value as f64,
            ))
        } else {
            None
        }
    }
}
