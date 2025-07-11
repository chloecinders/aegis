use crate::{executor::primitives::traits::PrimitiveValue, parser::Expr};

#[derive(Debug, Clone)]
pub struct BoolPrimitive {
    pub value: bool,
}

impl PrimitiveValue<bool> for BoolPrimitive {
    fn display(&self) -> String {
        self.value.to_string()
    }

    fn from_value_to_expr(value: bool) -> Expr {
        Expr::Bool(Self { value })
    }

    fn new(value: bool) -> Self {
        Self { value }
    }
}
