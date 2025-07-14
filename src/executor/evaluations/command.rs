use std::{
    cell::RefCell,
    process::{Command, Stdio},
    rc::Rc,
};

use crate::executor::{Scope, executor::ExecutorError};

pub fn evaluate_command(
    scope: Rc<RefCell<Scope>>,
    name: String,
    args: Vec<String>,
) -> Result<String, ExecutorError> {
    let env = scope.borrow().environment;

    if let Some((_, cmd)) = env.cmds.iter().find(|(c, _)| *c == name) {
        (*cmd)(args);

        Ok(String::default())
    } else if let Some(exe) = env.find_executable(name) {
        let mut child = match Command::new(exe.file_name().unwrap())
            .args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{e:?}");
                return Err(ExecutorError::Placeholder);
            }
        };

        match child.wait() {
            Ok(i) => i,
            Err(_) => return Err(ExecutorError::Placeholder),
        };

        // Ok(format!("{}", code.code().unwrap_or(1)))
        Ok(String::default())
    } else {
        Err(ExecutorError::Placeholder)
    }
}
