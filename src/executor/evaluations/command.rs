use std::{
    io::Stdout,
    process::{Child, Command, Stdio},
};

use crate::{
    executor::{Scope, executor::ExecutorError},
    parser::Expr,
};

pub fn evaluate_command(
    scope: &mut Scope,
    name: String,
    args: Vec<String>,
) -> Result<String, ExecutorError> {
    let env = scope.environment;

    if let Some(exe) = env.find_executable(name) {
        let mut child = match Command::new(exe.file_name().unwrap())
            .args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => return Err(ExecutorError::Placeholder),
        };

        let code = match child.wait() {
            Ok(i) => i,
            Err(_) => return Err(ExecutorError::Placeholder),
        };

        Ok(format!("{}", code.code().unwrap_or(1)))
    } else {
        Err(ExecutorError::Placeholder)
    }
}
