use crate::{
    executor::primitives::{
        IntPrimitive,
        traits::{PrimitiveValue, bin_ops::PrimitiveBinOps},
    },
    parser::Expr,
};

#[derive(Debug)]
pub struct FloatPrimitive {
    pub value: f64,
}

impl PrimitiveValue<f64> for FloatPrimitive {
    fn display(&self) -> String {
        self.value.to_string()
    }

    fn from_value_to_expr(value: f64) -> Expr {
        Expr::Float(Self { value })
    }
}

impl PrimitiveBinOps for FloatPrimitive {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn bin_add(&self, other: &dyn PrimitiveBinOps) -> Option<Expr> {
        if let Some(other) = other.as_any().downcast_ref::<FloatPrimitive>() {
            Some(FloatPrimitive::from_value_to_expr(self.value + other.value))
        } else if let Some(other) = other.as_any().downcast_ref::<IntPrimitive>() {
            Some(FloatPrimitive::from_value_to_expr(
                self.value + other.value as f64,
            ))
        } else {
            None
        }
    }
}
