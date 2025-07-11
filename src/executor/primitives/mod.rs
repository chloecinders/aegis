pub mod traits;

mod int;
pub use int::IntPrimitive;

mod float;
pub use float::FloatPrimitive;

mod string;
pub use string::StringPrimitive;

mod bool;
pub use bool::BoolPrimitive;
