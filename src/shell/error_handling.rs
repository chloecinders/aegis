// @TODO: Remove
#![allow(dead_code)]
pub struct ShellError {
    pub input: &'static str,
    pub pos: usize,
    pub title: String,
}

pub fn pretty_print_error(_err: ShellError) {
    todo!()
}
