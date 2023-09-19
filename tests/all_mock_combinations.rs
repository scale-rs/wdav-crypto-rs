use std::error::Error;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, ExitStatus};
use test_binary::TestBinary;

fn manifest_path_for_subdir(subdir: &str) -> PathBuf {
    PathBuf::from_iter(["testbins", subdir, "Cargo.toml"])
}

#[repr(transparent)]
#[derive(thiserror::Error, Debug)]
#[error("status:\n{status}")]
struct ExitStatusWrapped {
    status: ExitStatus,
}
impl ExitStatusWrapped {
    fn new(status: ExitStatus) -> Self {
        Self { status: status }
    }
}

fn run_main_under_subdir(subdir: &str) -> Result<(), Box<dyn Error>> {
    let manifest_path = manifest_path_for_subdir(subdir);
    // Even though the binary source is in `main.rs`, the executable will be called the same as its
    // crate (and as its project folder) - as given in `subdir`.
    let mut binary = TestBinary::relative_to_parent(subdir, &manifest_path);
    // @TODO if we don't paralellize the tested feature combinations fully, then apply
    // .with_feature(...) once per feature; re-build in the same folder (per the same
    // channel/sequence of run, but stop on the first error (or warning), unless configured
    // otherwise.
    match binary.with_profile("dev").build() {
        Ok(path) => {
            let mut command = Command::new(path);
            command.env("RUST_TEST_TIME_INTEGRATION", "3600000");
            let output = command.output();
            match output {
                Ok(output) => {
                    // If we have both non-empty stdout and stderr, print stdout first and stderr
                    // second. That way the developer is more likely to notice (and there is less to
                    // scroll up).
                    let mut stdout = io::stdout().lock();
                    stdout.write_all(&output.stdout)?;
                    stdout.flush()?;

                    if output.status.success() && output.stderr.is_empty() {
                        Ok(())
                    } else {
                        if !output.stderr.is_empty() {
                            let mut stderr = io::stderr().lock();
                            stderr.write_all(&output.stderr)?;
                            stderr.flush()?;
                        }
                        Err(Box::new(ExitStatusWrapped::new(output.status)))
                    }
                }
                Err(e) => Err(Box::new(e)),
            }
        }
        Err(e) => Err(Box::new(e)),
    }
}

#[test]
pub fn run_all_mock_combinations() -> Result<(), Box<dyn Error>> {
    run_main_under_subdir("fs_mock_entry_mock")?;
    run_main_under_subdir("fs_mock_entry_real")?;
    Ok(())
}
