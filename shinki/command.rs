use std::{io, path::Path, process::{Command, Output}};

pub fn describe(command: &Command) -> String {
    let program = command.get_program();
    let name = Path::new(program).file_name().unwrap_or(program);
    let lookup = if cfg!(windows) { "where.exe" } else { "which" };
    let found = match Command::new(lookup).arg(name).output() {
        Ok(output) => format!("status={} stdout={:?} stderr={:?}", output.status,
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()),
        Err(error) => format!("{lookup} could not run: {error}"),
    };
    format!("command={command:?}; executable={}; {lookup} {name:?}: {found}", Path::new(program).display())
}

pub fn output(command: &mut Command) -> io::Result<Output> {
    command.output().map_err(|error| {
        io::Error::new(error.kind(), format!("{}; spawn failed: {error}", describe(command)))
    })
}
