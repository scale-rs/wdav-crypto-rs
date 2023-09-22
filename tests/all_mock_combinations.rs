#![feature(can_vector, read_buf, write_all_vectored)]
//! Any `S` generic parameter is for [String]/[str] slice-like type, used for accepting names of
//! directories, files/binary crates, features...
//!
//! Any `B` generic parameter is for [BinaryCrateName]. That's separate from `S` because of
//! lifetimes and borrowing.

use core::borrow::Borrow;
use core::time::Duration;
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

fn manifest_path_for_subdir<S>(parent_dir: &S, sub_dir: &S) -> PathBuf
where
    S: Borrow<str> + ?Sized,
{
    PathBuf::from_iter([parent_dir.borrow(), sub_dir.borrow(), "Cargo.toml"])
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

pub enum BinaryCrateName<'b, B>
where
    B: 'b + ?Sized,
    &'b B: Borrow<str>,
{
    /// The binary (executable) name is the same as `[package]` name in `Cargo.toml`. (That's the
    /// default binary crate, and its source code is (by
    /// default/[auto-discovery](https://doc.rust-lang.org/nightly/cargo/reference/cargo-targets.html#target-auto-discovery)))
    /// in `src/main.rs`.)
    Main,
    /// Non-default binary name, whose source code is (by default) under
    /// [`src/bin/`](https://doc.rust-lang.org/nightly/cargo/reference/cargo-targets.html#binaries).
    /// The binary (executable) name is (by default/
    /// [auto-discovery](https://doc.rust-lang.org/nightly/cargo/reference/cargo-targets.html#target-auto-discovery))
    /// the same as its source file name (excluding `.rs`; add `.exe` on Windows).
    Other(&'b B),
}
impl<'b, B> BinaryCrateName<'b, B>
where
    B: 'b + ?Sized,
    &'b B: Borrow<str>,
{
    fn borrow(&self) -> &str {
        match self {
            Self::Main => "main",
            Self::Other(o) => o.borrow(),
        }
    }
}

fn bb(b: BinaryCrateName<str>) {}

type DynErr = Box<dyn Error>;
type DynErrResult<T> = Result<T, DynErr>;

fn spawn_main_under_subdir<'s, 'b, S, B>(
    parent_dir: &S,
    sub_dir: &S,
    binary_crate: &BinaryCrateName<'b, B>,
    features: impl IntoIterator<Item = &'s S>,
) -> DynErrResult<Child>
where
    S: Borrow<str> + 's + ?Sized,
    B: 'b + ?Sized,
    &'b B: Borrow<str>,
{
    let manifest_path = manifest_path_for_subdir(parent_dir, sub_dir);
    let binary_crate = binary_crate.borrow();
    let mut binary = TestBinary::relative_to_parent(binary_crate, &manifest_path);
    binary.with_profile("dev");
    for feature in features {
        binary.with_feature(feature.borrow());
    }
    // @TODO if we don't paralellize the tested feature combinations fully, then apply
    // .with_feature(...) once per feature; re-build in the same folder (per the same
    // channel/sequence of run, but stop on the first error (or warning), unless configured
    // otherwise.
    match binary.build() {
        Ok(path) => {
            let mut command = Command::new(path);
            //command.env("RUST_TEST_TIME_INTEGRATION", "3600000");
            println!(
                "Starting a process under {}/ binary crate {}.",
                sub_dir.borrow(),
                binary_crate
            );
            return Ok(command.spawn()?);
        }
        Err(e) => Err(Box::new(e)),
    }
}

/// Result of [Child]'s `id()` method. NOT a (transparent) single item struct, because we don't use
/// [u32] for anything else here.
type ChildId = u32;

/// NOT [std::collections::HashSet], because that doesn't allow mutable access to items (otherwise
/// their equality and hash code could change, and HashSet's invariants wouldn't hold true anymore).
///
/// Keys are results of [Child]'s `id()` method.
///
/// We could use [Vec], but child processes get removed incrementally => O(n^2).
type GroupOfChildren = HashMap<ChildId, Child>;

/// Iterate over the given children max. once. Take the first finished child (if any), and return
/// its process ID and exit status.
///
/// The [ChildId] is child process ID of the finished process.
///
/// Beware: [Ok] of [Some] CAN contain [ExitStatus] _NOT_ being OK!
fn finished_child(children: &mut GroupOfChildren) -> DynErrResult<Option<(ChildId, ExitStatus)>> {
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

/// Indicate when to end an execution of parallel tasks in the same group, or a sequence of groups.
pub enum ExecutionEnd {
    /// Stop any and all active tasks on first failure. Stop them without reporting any output from
    /// them (except for the failed task). Don't start any subsequent task(s).
    OnFailureStopAll,
    /// On failure of any tasks that have already started, wait until all other parallel tasks
    /// finish, too. Report output from all of them. Potentially reorder their outputs, so that
    /// outputs of any failed task(s) will be at the end. Don't start any subsequent tasks.
    OnFailureFinishActive,
    /// Run all group(s) and all task(s) in each group. Wait for all of them, even if any of them
    /// fail.
    ProcessAll,
}

impl ExecutionEnd {
    pub fn after_errors(&self, errors: Vec<DynErr>) -> SpawningModeAndErrors {
        panic!()
    }
}

/// Mode of handling task life cycle.
pub enum SpawningMode {
    /// Default (until there is any error, or until we finish all tasks).
    ProcessAll,
    /// Finish active tasks, collect their output. Don't start any new ones.
    FinishActive,
    /// Stop any and all active tasks. Ignore their output (except for the task that has failed and
    /// that triggered this mode).
    StopAll,
}

pub struct SpawningModeAndErrors {
    pub mode: SpawningMode,
    pub errors: Vec<DynErr>,
}

impl SpawningModeAndErrors {
    pub fn after_error(self, until: ExecutionEnd, err: DynErr) -> Self {
        match (&self, &until) {
            (
                Self {
                    mode: SpawningMode::ProcessAll,
                    errors: _,
                },
                _,
            ) => {
                panic!()
            }
            (_, _) => {
                panic!()
            }
        }
    }
}

/// Run a group of parallel binary crate invocations. Each item (a tuple) of the group consists of
/// two fields:
/// - subdirectory, and
/// - crate feature name(s), if any.
///
/// All entries are run in parallel. It's an error if two or more entries have the same subdirectory
/// name.
pub fn run_parallel_single_tasks<'s, S, FEATURES, TASKS>(
    parent_dir: &S,
    tasks: TASKS,
    until: ExecutionEnd,
) where
    S: Borrow<str> + 's + ?Sized,
    FEATURES: IntoIterator<Item = S>,
    TASKS: IntoIterator<Item = (&'s S /*binary crate name*/, FEATURES)>,
{
}

/// Run a sequence of the same binary crate (under the same sub dir) invocation(s), but each
/// invocation with possibly different combinations of crate features.
///
/// The tasks are run in sequence, but their output may be reordered, to have any non-empty `stderr`
/// at the end.
pub fn run_sequence_single_tasks<
    's,
    S,
    #[allow(non_camel_case_types)] FEATURE_SET,
    #[allow(non_camel_case_types)] FEATURE_SETS,
>(
    parent_dir: &S,
    sub_dir: &S,
    feature_sets: FEATURE_SETS,
    until: ExecutionEnd,
) where
    S: Borrow<str> + 's + ?Sized,
    FEATURE_SET: IntoIterator<Item = &'s S>,
    FEATURE_SETS: IntoIterator<Item = FEATURE_SET>,
{
}

/// Run multiple sequences, where each sequence step runs a group of task(s) in parallel.
///
/// Their output may be reordered, to have any non-empty `stderr` at the end.
pub fn run_parallel_sequences_of_parallel_tasks<
    's,
    S,
    #[allow(non_camel_case_types)] FEATURE_SET,
    #[allow(non_camel_case_types)] PARALLEL_TASKS,
    SEQUENCE,
    SEQUENCES,
>(
    parent_dir: &S,
    sequences: SEQUENCES,
    until: ExecutionEnd,
) where
    S: Borrow<str> + 's + ?Sized,
    FEATURE_SET: IntoIterator<Item = &'s S /* feature*/>,
    PARALLEL_TASKS: IntoIterator<Item = (&'s S /* binary crate name*/, FEATURE_SET)>,
    SEQUENCE: IntoIterator<Item = PARALLEL_TASKS>,
    SEQUENCES: IntoIterator<Item = SEQUENCE>,
{
}

/// Start a number of parallel child process(es) - tasks, all under the same `parent_dir`.
///
/// This does NOT have a [SpawningMode] parameter - we behave as if under
/// [SpawningMode::ProcessAll].
///
/// This does NOT check for exit status/stderr of any spawn child processes. It only checks if the
/// actual spawning itself (system call) was successful. If all spawn successfully, then the
/// [SpawningMode] of the result tuple is [SpawningMode::ProcessAll]. Otherwise the [SpawningMode]
/// part of the result tuple is either [SpawningMode::FinishActive] or [SpawningMode::StopAll],
/// depending on the given `until` ([ExecutionEnd]).
fn group_start<
    's,
    'b,
    S,
    B,
    #[allow(non_camel_case_types)] FEATURE_SET,
    #[allow(non_camel_case_types)] PARALLEL_TASKS,
>(
    parent_dir: &S,
    tasks: PARALLEL_TASKS,
    until: ExecutionEnd,
) -> DynErrResult<(GroupOfChildren, SpawningModeAndErrors)>
where
    S: Borrow<str> + 's + ?Sized,
    B: 'b + ?Sized,
    &'b B: Borrow<str>,
    FEATURE_SET: IntoIterator<Item = &'s S /* feature */>,
    PARALLEL_TASKS: IntoIterator<
        Item = (
            &'s S, /* sub_dir */
            &'b BinaryCrateName<'b, B>,
            FEATURE_SET,
        ),
    >,
{
    let mut children = GroupOfChildren::new();
    for (sub_dir, binary_crate, features) in tasks {
        let child_or_err = spawn_main_under_subdir(parent_dir, sub_dir, binary_crate, features);

        match child_or_err {
            Ok(child) => children.insert(child.id(), child),
            Err(err) => {
                for (_, mut other_child) in children {
                    let _ = other_child.kill();
                }
                return Err(err);
            }
        };
    }
    panic!()
}

fn group_life_cycle_step(
    group: GroupOfChildren,
    mode: SpawningModeAndErrors,
    until: ExecutionEnd,
) -> (GroupOfChildren, SpawningModeAndErrors) {
    panic!()
}

fn run_sub_dirs<'s, 'b, S, B>(
    parent_dir: &S,
    sub_dirs: impl IntoIterator<Item = &'s S>,
    binary_crate: BinaryCrateName<'b, B>,
) -> DynErrResult<()>
where
    S: Borrow<str> + 's + ?Sized,
    B: 'b + ?Sized,
    &'b B: Borrow<str>,
{
    let mut children = GroupOfChildren::new();
    for sub_dir in sub_dirs {
        let child_or_err = spawn_main_under_subdir(parent_dir, &sub_dir, &binary_crate, []);

        match child_or_err {
            Ok(child) => children.insert(child.id(), child),
            Err(err) => {
                for (_, mut other_child) in children {
                    let _ = other_child.kill();
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
                    continue;
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
        BinaryCrateName::Main,
    )
}
