use crate::{executor::primitives::traits::PrimitiveValue, parser::Expr};

#[derive(Debug, Clone)]
pub struct StringPrimitive {
    pub value: String,
}

impl PrimitiveValue<String> for StringPrimitive {
    fn display(&self) -> String {
        self.value.to_string()
    }

    fn from_value_to_expr(value: String) -> Expr {
        Expr::String(Self { value })
    }

    fn new(value: String) -> Self {
        Self { value }
    }
}
