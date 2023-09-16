use core::fmt::{Display, Formatter, Result as FmtResult};
use std::error::Error;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Output};
use test_binary::TestBinary;

#[repr(transparent)]
#[derive(Debug)]
struct DisplayableBytes {
    bytes: Vec<u8>,
}
impl Display for DisplayableBytes {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match std::str::from_utf8(&self.bytes) {
            Ok(str) => str.fmt(f),
            Err(_) => f.write_fmt(format_args!("{:?}", self.bytes)),
        }
    }
}

fn manifest_path_for_subdir(subdir: &str) -> PathBuf {
    PathBuf::from_iter(["testbins", subdir, "Cargo.toml"])
}

#[derive(thiserror::Error, Debug)]
#[error("status:\n{status}\n\nstdout:\n{stdout}\n\nstderr:\n{stderr}")]
struct LikeOutput {
    status: ExitStatus,
    stdout: DisplayableBytes,
    stderr: DisplayableBytes,
}
impl LikeOutput {
    fn new(output: Output) -> Self {
        Self {
            status: output.status,
            stdout: DisplayableBytes {
                bytes: output.stdout,
            },
            stderr: DisplayableBytes {
                bytes: output.stderr,
            },
        }
    }
}

fn run_main_under_subdir(subdir: &str) -> Result<(), Box<dyn Error>> {
    let manifest_path = manifest_path_for_subdir(subdir);
    // Even though the binary source is in `main.rs`, the executable will be called the same as its
    // crate (and as its project folder) - as given in `subdir`.
    let mut binary = TestBinary::relative_to_parent(subdir, &manifest_path);
    match binary.with_profile("dev").build() {
        Ok(path) => {
            let output = Command::new(path).output();
            match output {
                Ok(output) => {
                    if output.status.success() && output.stderr.is_empty() {
                        println!(
                            "{}",
                            DisplayableBytes {
                                bytes: output.stdout
                            }
                        );
                        println!(
                            "{}",
                            DisplayableBytes {
                                bytes: output.stderr
                            }
                        );
                        Ok(())
                    } else {
                        Err(Box::new(LikeOutput::new(output)))
                    }
                }
                Err(e) => Err(Box::new(e)),
            }
        }
        Err(e) => Err(Box::new(e)),
    }
}

#[test]
pub fn all_mock_combinations() -> Result<(), Box<dyn Error>> {
    run_main_under_subdir("fs_mock_entry_mock")?;
    run_main_under_subdir("fs_mock_entry_real")?;
    Ok(())
}
