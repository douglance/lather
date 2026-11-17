use crate::model::{Error, Result, WithCode};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

const MAX_OUTPUT: u64 = 8 * 1024 * 1024;

pub struct Output {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    pub fn checked(self, command: &str) -> Result<String> {
        if self.status.success() {
            Ok(self.stdout)
        } else {
            Err(Error::new(
                "COMMAND_FAILED",
                format!("{command}: {}", self.diagnostic()),
            ))
        }
    }

    pub fn diagnostic(&self) -> String {
        let text = if self.stderr.trim().is_empty() {
            &self.stdout
        } else {
            &self.stderr
        };
        text.trim()
            .chars()
            .rev()
            .take(1200)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }
}

pub fn run(root: &Path, program: &str, args: &[&str], timeout: Duration) -> Result<Output> {
    let stdout = tempfile::tempfile().code("RUNNER_IO")?;
    let stderr = tempfile::tempfile().code("RUNNER_IO")?;
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(stdout.try_clone().code("RUNNER_IO")?)
        .stderr(stderr.try_clone().code("RUNNER_IO")?)
        .env("NO_COLOR", "1")
        .env("CARGO_TERM_COLOR", "never")
        .env("GIT_OPTIONAL_LOCKS", "0");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|error| {
        let code = if error.kind() == std::io::ErrorKind::NotFound {
            "TOOL_MISSING"
        } else {
            "COMMAND_START"
        };
        Error::new(code, format!("{program}: {error}"))
    })?;
    let status = match child.wait_timeout(timeout) {
        Ok(Some(status)) => status,
        result => {
            #[cfg(unix)]
            let _ = Command::new("kill")
                .args(["-KILL", "--", &format!("-{}", child.id())])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = child.kill();
            let _ = child.wait();
            return Err(match result {
                Err(error) => Error::new("RUNNER_IO", format!("{program}: {error}")),
                _ => Error::new(
                    "CHECK_TIMEOUT",
                    format!("{program} exceeded {} seconds", timeout.as_secs()),
                ),
            });
        }
    };
    Ok(Output {
        status,
        stdout: bounded_read(stdout)?,
        stderr: bounded_read(stderr)?,
    })
}

fn bounded_read(mut file: std::fs::File) -> Result<String> {
    if file.metadata().code("RUNNER_IO")?.len() > MAX_OUTPUT {
        return Err(Error::new("OUTPUT_LIMIT", "Check output exceeded 8 MiB"));
    }
    file.seek(SeekFrom::Start(0)).code("RUNNER_IO")?;
    let mut bytes = Vec::new();
    file.take(MAX_OUTPUT + 1)
        .read_to_end(&mut bytes)
        .code("RUNNER_IO")?;
    if bytes.len() as u64 > MAX_OUTPUT {
        return Err(Error::new("OUTPUT_LIMIT", "Check output exceeded 8 MiB"));
    }
    String::from_utf8(bytes).code("OUTPUT_UNPARSEABLE")
}
