#![feature(can_vector, read_buf, write_all_vectored)]

use core::time::Duration;
use http::header::IntoIter;
use std::collections::HashMap;
use std::error::Error;
use std::io::{self, IoSlice, IoSliceMut, Read, Result as IoResult, Write};
use std::ops::Deref;
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus};
use std::thread;
use test_binary::TestBinary;

const INTERMEDIARY_DIR: &'static str = "testbins";

/// Based on
/// https://www.baeldung.com/linux/pipe-buffer-capacity#:~:text=In%20Linux%2C%20pipe%20buffer%20capacity,page%20size%20of%204%2C096%20bytes)
/// and https://unix.stackexchange.com/questions/11946/how-big-is-the-pipe-buffer.
const BUFFER_SIZE: usize = 16 * 4096;

/// How long to sleep before checking again whether any child process(es) finished.
const SLEEP_BETWEEN_CHECKING_CHILDREN: Duration = Duration::from_millis(50);

fn manifest_path_for_subdir(parent_dir: &str, sub_dir: &str) -> PathBuf {
    PathBuf::from_iter([parent_dir, sub_dir, "Cargo.toml"])
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

type DynErrResult<T> = Result<T, Box<dyn Error>>;

fn spawn_main_under_subdir(
    parent_dir: &str,
    sub_dir: &str, /*, features: impl IntoIterator<Item = F>*/
) -> DynErrResult<Child> {
    let manifest_path = manifest_path_for_subdir(parent_dir, sub_dir);
    // Even though the binary source is in `main.rs`, the executable will be called the same as its
    // crate (and as its project folder) - as given in `subdir`.
    let mut binary = TestBinary::relative_to_parent(sub_dir, &manifest_path);
    // @TODO if we don't paralellize the tested feature combinations fully, then apply
    // .with_feature(...) once per feature; re-build in the same folder (per the same
    // channel/sequence of run, but stop on the first error (or warning), unless configured
    // otherwise.
    match binary.with_profile("dev").build() {
        Ok(path) => {
            let mut command = Command::new(path);
            command.env("RUST_TEST_TIME_INTEGRATION", "3600000");
            println!("Starting a process for {}.", sub_dir);
            return Ok(command.spawn()?);
        }
        Err(e) => Err(Box::new(e)),
    }
}

/// NOT [std::collections::HashSet], because that doesn't allow mutable access to items (otherwise
/// their equality and hash code could change, and HashSet's invariants wouldn't hold true anymore).
///
/// Keys are results of [Child]'s `id()` method.
///
/// We could use [Vec], but child get removed incrementally => O(n^2).
type Children = HashMap<u32, Child>;

/// Iterate over the given children max. once. Take the first finished child (if any), and return
/// its process ID and exit status.
///
/// The [u8] part of the `Ok(Some((u8,ExitStatus)))` variant is child process ID of the finished
/// process.
///
/// [Ok] of [Some] CAN contain [ExitStatus] _NOT_ being OK!
fn finished_child(children: &mut Children) -> DynErrResult<Option<(u32, ExitStatus)>> {
    for (child_id, child) in children.iter_mut() {
        let opt_status_or_err = child.try_wait();

        match opt_status_or_err {
            Ok(Some(exit_status)) => {
                return Ok(Some((*child_id, exit_status)));
            }
            Ok(None) => {}
            Err(err) => return Err(Box::new(err)),
        }
    }
    Ok(None)
}

fn copy_all_bytes_classic(
    out: &mut impl Write,
    inp: &mut impl Read,
    buffer: &mut [u8],
) -> IoResult<usize> {
    let mut total_len = 0usize;

    loop {
        let len_read = inp.read(buffer)?;
        if len_read == 0 {
            break Ok(total_len);
        }

        out.write(&buffer[0..len_read])?;
        total_len += len_read;
    }
}

fn copy_all_bytes_vectored(
    out: &mut impl Write,
    inp: &mut impl Read,
    buffer: &mut [u8],
) -> IoResult<usize> {
    let slice_in = IoSliceMut::new(buffer);
    let mut slice_in_wrapped = [slice_in];
    let mut total_len = 0usize;

    loop {
        let len_read = inp.read_vectored(&mut slice_in_wrapped)?;
        if len_read == 0 {
            break Ok(total_len);
        }

        let slice_from = IoSlice::new(&slice_in_wrapped[0].deref()[0..len_read]);
        out.write_all_vectored(&mut [slice_from])?;
        total_len += len_read;
    }
}

/// Copy, through a buffer of [BUFFER_SIZE] bytes. Return the total length copied (on success).
fn copy_all_bytes(out: &mut impl Write, inp: &mut impl Read) -> IoResult<usize> {
    let mut buffer = [0u8; BUFFER_SIZE];

    if inp.is_read_vectored() && out.is_write_vectored() {
        copy_all_bytes_vectored(out, inp, &mut buffer)
    } else {
        copy_all_bytes_classic(out, inp, &mut buffer)
    }
}

/// Indicate when to end an execution of parallel runs in the same batch, or a sequence of batches.
pub enum ExecutionEnd {
    /// Stop the current batch on first failure. Kill any other parallel runs, without reporting any
    /// output from them. Don't start any subsequent batch(es).
    OnFirstFailure,
    /// On failure of any runs that have already started, wait until all other parallel runs finish,
    /// too. Report output from all of them. However, don't start any subsequent runs.
    FinishBatch,
    /// Run all batch(es) and all runs in each batch. Wait for all of them, even if any of them
    /// fails.
    FinishAll,
}

/// Run a batch of parallel binary crate invocations. Each item (a tuple) of the `batch` consists of
/// two fields:
/// - subdirectory, and
/// - crate feature name(s), if any.
///
/// All entries are run in parallel. It's an error if two or more entries have the same subdirectory
/// name.
pub fn run_parallel_single_tasks<SUBDIR, FEATURE, FEATURES>(
    parent_dir: &str,
    tasks: impl IntoIterator<Item = (SUBDIR, FEATURES)>,
) where
    SUBDIR: AsRef<str>,
    FEATURE: AsRef<str>,
    FEATURES: IntoIterator<Item = FEATURE>,
{
}

/// Run a sequence of same binary crate (under the same sub dir) invocation(s), but each invocation
/// with possibly different combinations of crate features.
pub fn run_sequence_single_tasks<
    SUBDIR,
    FEATURE,
    #[allow(non_camel_case_types)] FEATURE_SET,
    #[allow(non_camel_case_types)] FEATURE_SETS,
>(
    parent_dir: &str,
    sub_dir: SUBDIR,
    feature_sets: FEATURE_SETS,
) where
    SUBDIR: AsRef<str>,
    FEATURE: AsRef<str>,
    FEATURE_SET: IntoIterator<Item = FEATURE>,
    FEATURE_SETS: IntoIterator<Item = FEATURE_SET>,
{
}

/// Run multiple sequences of batches in parallel.
pub fn run_parallel_sequences_of_parallel_tasks<
    SUBDIR,
    FEATURE,
    #[allow(non_camel_case_types)] FEATURE_SET,
    #[allow(non_camel_case_types)] PARALLEL_TASKS,
    SEQUENCE,
    SEQUENCES,
>(
    parent_dir: &str,
    sequences: SEQUENCES,
) where
    SUBDIR: AsRef<str>,
    FEATURE: AsRef<str>,
    FEATURE_SET: IntoIterator<Item = FEATURE>,
    PARALLEL_TASKS: IntoIterator<Item = (SUBDIR, FEATURE_SET)>,
    SEQUENCE: IntoIterator<Item = PARALLEL_TASKS>,
    SEQUENCES: IntoIterator<Item = SEQUENCE>,
{
}

fn run_sub_dirs<S>(parent_dir: &str, sub_dirs: impl IntoIterator<Item = S>) -> DynErrResult<()>
where
    S: AsRef<str>,
{
    let mut children = Children::new();
    for sub_dir in sub_dirs {
        let child_or_err = spawn_main_under_subdir(parent_dir, sub_dir.as_ref());

        match child_or_err {
            Ok(child) => children.insert(child.id(), child),
            Err(err) => {
                for (mut _other_id, mut other_child) in children {
                    let _ignored_result = other_child.kill();
                }
                return Err(err);
            }
        };
    }

    loop {
        let finished_result = finished_child(&mut children);
        match finished_result {
            Ok(Some((child_id, status))) => {
                let child = children.remove(&child_id).unwrap();
                let mut stdout = io::stdout().lock();
                let mut stderr = io::stderr().lock();

                // If we have both non-empty stdout and stderr, print stdout first and stderr
                // second. That way the developer is more likely to notice (and there is less
                // vertical distance to scroll up).
                if let Some(mut child_out) = child.stdout {
                    copy_all_bytes(&mut stdout, &mut child_out)?;
                }
                let err_len = if let Some(mut child_err) = child.stderr {
                    copy_all_bytes(&mut stderr, &mut child_err)?
                } else {
                    0
                };

                if status.success() && err_len == 0 {
                    break Ok(());
                } else {
                    stderr.flush()?;
                    break Err(Box::new(ExitStatusWrapped::new(status)));
                }
            }
            Ok(None) => {
                if children.is_empty() {
                    break Ok(());
                } else {
                    thread::sleep(SLEEP_BETWEEN_CHECKING_CHILDREN);
                    continue;
                }
            }
            Err(err) => {
                break Err(err);
            }
        }
    }
}

#[test]
pub fn run_all_mock_combinations() -> DynErrResult<()> {
    run_sub_dirs(
        INTERMEDIARY_DIR,
        vec!["fs_mock_entry_mock", "fs_mock_entry_real"],
    )
}
