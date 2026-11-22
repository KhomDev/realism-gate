//! `realism-gate verify-fix`: run a PoC on the unpatched tree and on the
//! patched tree.
//!
//! The unpatched run has to pass (the bug reproduces). The patched run has to
//! fail (the fix kills that PoC). A PoC that fails on both sides does not
//! show a fix. A compile error is not a failed PoC.
//!
//! The verdict is the JSON forge writes to stdout (`--json`), read as JSON,
//! together with the process exit code. Text in the log, including a line
//! that contains `"success": false`, is not a verdict.
//!
//! Forge runs on this machine. A project under review is untrusted: it can
//! enable `ffi`, grant itself `fs_permissions`, and make forge open a
//! connection. This command refuses to run when any `foundry.toml` in the tree
//! sets `ffi = true`, or when any `fs_permissions` entry is not exactly
//! `access = "read"` (write access would let the first PoC run rewrite the
//! other tree's or the global config for the second run). The root project,
//! the `--patched` tree, the copied tree after a `--patch` is applied and
//! Foundry's global config (`~/.foundry/foundry.toml`, or
//! `%USERPROFILE%\.foundry\foundry.toml`) are parsed as TOML before the tree
//! is used. The global config and both copies are parsed again immediately
//! before each forge start, together with the `.env` check.
//! `FOUNDRY_CONFIG` is pinned to the absolute path of a checked config in the
//! copy (a minimal one is created when the tree has none), and a `.env` in a
//! copied tree is refused: Foundry loads dotenv after it starts and would
//! otherwise redirect the variable to a config this command never parsed. A
//! `foundry.toml` in an ancestor directory of the temp copy is not checked;
//! `docs/verify-fix.md` states that residual. The environment variable
//! `FOUNDRY_FFI` is not a control either: Foundry documents it as an explicit
//! exception to env overrides, so this command does not rely on setting it.
//!
//! Path arguments that name a file are resolved from the caller's current
//! directory before the temp copy becomes the process working directory:
//! `--patch ../fix.diff` and a `--forge` value containing a separator keep
//! working. A bare `--forge forge` keeps PATH lookup.
//!
//! The host decision is part of this function, not only of the CLI: a
//! [`Request`] with no [`HostPolicy`] is refused before a copy is made or a
//! forge process is started, so a library caller cannot skip the check. See
//! `docs/verify-fix.md`.

use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Request {
    /// The Foundry test file, relative to `root` (or absolute).
    pub poc: PathBuf,
    /// A unified diff to apply to a copy of `root`.
    pub patch: Option<PathBuf>,
    /// A directory that already contains the patched sources.
    pub patched_dir: Option<PathBuf>,
    pub match_test: Option<String>,
    /// The forge binary. Tests pass a stand-in.
    pub forge: PathBuf,
    /// The project forge should run in. The unpatched run is a copy of this
    /// tree. Defaults to the current directory at the call site.
    pub root: PathBuf,
    /// Whether the operator has accepted running forge here. `None` is the
    /// absence of an attestation and is refused by [`verify`] before any copy
    /// is made or any process is started.
    pub host: Option<HostPolicy>,
}

/// The refusal a caller gets without an attestation. The CLI prints this, and
/// a library caller that leaves [`Request::host`] as `None` gets the same.
const HOST_REFUSAL: &str = "verify-fix will not run an untrusted forge project on the host. \
     Run it in a container (ffi off, network only to the pinned RPC) and pass --sandbox, \
     or pass --allow-host to run here anyway. See docs/verify-fix.md.";

/// Who decided it is acceptable to execute the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPolicy {
    /// Confined: a container, or `--sandbox` attesting one. `ffi` is still
    /// refused.
    Sandbox,
    /// `--allow-host`. The operator accepted host execution. `ffi` is still
    /// refused.
    AllowHost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The PoC passed on the unpatched tree and failed on the patched tree.
    FixHolds { detail: String },
    /// The PoC still passed on the patched tree.
    FixDoesNotHold { detail: String },
    /// The PoC did not pass on the unpatched tree. A later failure is not a fix.
    BugDoesNotReproduce { detail: String },
}

#[derive(Debug)]
pub struct VerifyError(pub String);

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

struct Side {
    passed: bool,
    detail: String,
}

/// Runs forge twice. `Ok` is a pass-or-fail of the fix; `Err` is forge
/// missing, a patch that did not apply, a project that is unsafe to run, or
/// a compile error (the PoC never ran).
pub fn verify(req: &Request) -> Result<Outcome, VerifyError> {
    verify_with_global_config(req, global_foundry_toml().as_deref())
}

/// The body of [`verify`]. `global_config` is Foundry's global
/// `foundry.toml` when it exists; the unit tests pass `None` so a developer's
/// own global config cannot change their result. The integration tests drive
/// [`verify`] through the binary with a fake home.
fn verify_with_global_config(
    req: &Request,
    global_config: Option<&Path>,
) -> Result<Outcome, VerifyError> {
    if req.patch.is_some() == req.patched_dir.is_some() {
        return Err(VerifyError(
            "pass exactly one of --patch FILE and --patched DIR".into(),
        ));
    }
    // The host decision is enforced here, not only in the CLI: a library
    // caller that supplies no policy is refused before any tree is copied.
    if req.host.is_none() {
        return Err(VerifyError(HOST_REFUSAL.into()));
    }
    // The global config and the root are checked before anything is copied
    // (fail fast); the --patched tree is checked before it is copied, and the
    // post-patch copy after the apply. The global config and both copies are
    // checked again immediately before each forge start below: the first PoC
    // run can rewrite the other copy or the global config unless its
    // fs_permissions are read-only, and a read-only config cannot do it.
    if let Some(path) = global_config {
        refuse_config_file(path, "Foundry's global config")?;
    }
    refuse_config_dir(&req.root, "the project in --root")?;

    let mut temps = TempDirs::default();
    let baseline = temps.push(copy_of(&req.root)?);
    refuse_config_dir(&baseline, "the copied project")?;
    refuse_dotenv(&baseline, "the copied project")?;
    let patched = if let Some(dir) = &req.patched_dir {
        if !dir.is_dir() {
            return Err(VerifyError(format!("{}: not a directory", dir.display())));
        }
        refuse_config_dir(dir, "the --patched tree")?;
        let copy = temps.push(copy_of(dir)?);
        refuse_config_dir(&copy, "the copied --patched tree")?;
        refuse_dotenv(&copy, "the copied --patched tree")?;
        copy
    } else {
        // Resolve from the caller's current directory now. `git apply` runs
        // with the temp copy as its working directory, where a relative
        // `../fix.diff` would name a different file (or none).
        let patch = absolute_from_cwd(req.patch.as_ref().expect("patch or dir"))?;
        if !patch.is_file() {
            return Err(VerifyError(format!("{}: no such patch", patch.display())));
        }
        let work = temps.push(copy_of(&req.root)?);
        apply_patch(&work, &patch)?;
        refuse_config_dir(&work, "the patched copy")?;
        refuse_dotenv(&work, "the patched copy")?;
        work
    };

    // Before the first run: both copies and the global config.
    revalidate_trees(
        global_config,
        &[
            (&baseline, "the copied project"),
            (&patched, "the patched tree"),
        ],
    )?;
    let before = run_forge(req, &baseline, "unpatched")?;
    // Again before the second run. This is the defense-in-depth catch for a
    // first run that rewrote the other copy or the global config; the
    // fs_permissions refusal above is the primary control.
    revalidate_trees(
        global_config,
        &[
            (&patched, "the patched tree"),
            (&baseline, "the copied project"),
        ],
    )?;
    let after = run_forge(req, &patched, "patched")?;
    let detail = format!("unpatched: {}\npatched: {}", before.detail, after.detail);
    if !before.passed {
        return Ok(Outcome::BugDoesNotReproduce { detail });
    }
    if after.passed {
        Ok(Outcome::FixDoesNotHold { detail })
    } else {
        Ok(Outcome::FixHolds { detail })
    }
}

/// `Command::new` on Windows cannot execute a `.bat` or `.cmd` (CreateProcess
/// returns error 193). Those extensions are the test double's stand-in for a
/// shell script. A real `forge` or `forge.exe` is launched directly.
fn forge_command(forge: &Path) -> Command {
    let ext = forge
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if cfg!(windows) && (ext == "bat" || ext == "cmd") {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(forge);
        cmd
    } else {
        Command::new(forge)
    }
}

// On Windows, executing a script that another test thread wrote a moment earlier
// can fail with a sharing violation (os error 32); Linux has the same shape as
// ETXTBSY (26). Only the parallel test suite can open that window, so the retry
// is compiled into test builds; a released binary keeps the single attempt.
#[cfg(test)]
fn spawn_with_retry(cmd: &mut Command) -> std::io::Result<Child> {
    let mut last = None;
    for _ in 0..10 {
        match cmd.spawn() {
            Ok(child) => return Ok(child),
            Err(e) if matches!(e.raw_os_error(), Some(26) | Some(32)) => {
                std::thread::sleep(Duration::from_millis(20));
                last = Some(e);
            }
            Err(e) => return Err(e),
        }
    }
    Err(last.expect("a retryable error was recorded"))
}

#[cfg(not(test))]
fn spawn_with_retry(cmd: &mut Command) -> std::io::Result<Child> {
    cmd.spawn()
}

/// The bytes one forge stream produced, and the exit status.
struct ForgeOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// Reads at most `cap` bytes. On the byte that would exceed the cap the
/// reader stops and sets `overflow`; dropping the pipe closes the read end,
/// so a flooding child gets a broken pipe instead of filling memory.
fn read_capped(mut reader: impl Read, cap: usize, overflow: Arc<AtomicBool>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if out.len() + n > cap {
                    let take = cap.saturating_sub(out.len());
                    out.extend_from_slice(&buf[..take]);
                    overflow.store(true, Ordering::SeqCst);
                    break;
                }
                out.extend_from_slice(&buf[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    out
}

fn stop_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Waits for a spawned forge with a wall-clock timeout and bounded streams.
/// A timeout or an overflow is a [`VerifyError`]: the verdict is never taken
/// from a run that did not finish or from a truncated log.
fn wait_forge(
    child: &mut Child,
    timeout: Duration,
    stdout_cap: usize,
    stderr_cap: usize,
) -> Result<ForgeOutput, VerifyError> {
    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");
    let out_overflow = Arc::new(AtomicBool::new(false));
    let err_overflow = Arc::new(AtomicBool::new(false));
    let (out_tx, out_rx) = mpsc::channel();
    let (err_tx, err_rx) = mpsc::channel();
    {
        let flag = Arc::clone(&out_overflow);
        std::thread::spawn(move || {
            let _ = out_tx.send(read_capped(stdout, stdout_cap, flag));
        });
    }
    {
        let flag = Arc::clone(&err_overflow);
        std::thread::spawn(move || {
            let _ = err_tx.send(read_capped(stderr, stderr_cap, flag));
        });
    }
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| VerifyError(format!("forge: {e}")))?
        {
            break status;
        }
        if out_overflow.load(Ordering::SeqCst) {
            stop_child(child);
            return Err(VerifyError(format!(
                "forge wrote more than {stdout_cap} bytes to stdout; it was killed and the verdict is not taken from a truncated log"
            )));
        }
        if err_overflow.load(Ordering::SeqCst) {
            stop_child(child);
            return Err(VerifyError(format!(
                "forge wrote more than {stderr_cap} bytes to stderr; it was killed and the verdict is not taken from a truncated log"
            )));
        }
        if start.elapsed() >= timeout {
            stop_child(child);
            return Err(VerifyError(format!(
                "forge did not finish within {} seconds; it was killed and there is no verdict",
                timeout.as_secs()
            )));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    // The direct child exited. A descendant can still hold a pipe open, so
    // the wait for the readers is bounded too.
    let stdout = out_rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| {
            VerifyError(
                "forge exited but its stdout pipe stayed open (a child process may still hold it); there is no verdict"
                    .into(),
            )
        })?;
    let stderr = err_rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| {
            VerifyError(
                "forge exited but its stderr pipe stayed open (a child process may still hold it); there is no verdict"
                    .into(),
            )
        })?;
    // The child can exit between the last overflow poll and `try_wait`. The
    // readers have finished by now, so a cap hit is visible here too; never
    // take a verdict from a stream that was truncated.
    if out_overflow.load(Ordering::SeqCst) {
        return Err(VerifyError(format!(
            "forge wrote more than {stdout_cap} bytes to stdout; the verdict is not taken from a truncated log"
        )));
    }
    if err_overflow.load(Ordering::SeqCst) {
        return Err(VerifyError(format!(
            "forge wrote more than {stderr_cap} bytes to stderr; the verdict is not taken from a truncated log"
        )));
    }
    Ok(ForgeOutput {
        status,
        stdout,
        stderr,
    })
}

fn run_forge(req: &Request, work: &Path, label: &str) -> Result<Side, VerifyError> {
    let poc = poc_arg(&req.poc, &req.root, work);
    let forge = forge_program(&req.forge)?;
    // Pin FOUNDRY_CONFIG to the absolute path of a config this command has
    // checked. Removing the variable is not enough: Foundry loads a `.env`
    // after it starts, and dotenvy does not override an existing variable, so
    // an inherited absolute value is what stops a `.env` from redirecting it.
    let config = pinned_config(work)?;
    let mut cmd = forge_command(&forge);
    cmd.current_dir(work)
        .arg("test")
        .arg("--match-path")
        .arg(&poc)
        .arg("--json")
        // FOUNDRY_FFI=false is not a control: Foundry documents the variable
        // as an explicit exception to env overrides. The TOML parse in
        // `verify` is what refuses ffi. No RPC URL is added; a fork test still
        // uses whatever URL the test names.
        .env("FOUNDRY_FFI", "false")
        .env("FOUNDRY_CONFIG", &config)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(name) = &req.match_test {
        cmd.arg("--match-test").arg(name);
    }
    let mut child = spawn_with_retry(&mut cmd).map_err(|e| {
        VerifyError(format!(
            "could not run {}: {e}. verify-fix needs forge on PATH, or --forge.",
            forge.display()
        ))
    })?;
    let output = wait_forge(
        &mut child,
        FORGE_TIMEOUT,
        FORGE_STDOUT_CAP,
        FORGE_STDERR_CAP,
    )?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let results = match parse_results(&stdout) {
        Ok(r) => r,
        Err(reason) => {
            return Err(VerifyError(format!(
                "forge did not run the PoC on the {label} tree (compile or usage error; {reason}):\n{}",
                first_lines(&format!("{stdout}\n{stderr}"), 20)
            )));
        }
    };
    let all_pass = results.iter().all(|p| *p);
    let any_fail = results.iter().any(|p| !*p);
    let exit_ok = output.status.success();
    if all_pass && exit_ok {
        Ok(Side {
            passed: true,
            detail: format!("{label} passed ({} test(s), exit 0)", results.len()),
        })
    } else if any_fail && !exit_ok {
        Ok(Side {
            passed: false,
            detail: format!(
                "{label} failed ({} of {} test(s), exit non-zero)",
                results.iter().filter(|p| !**p).count(),
                results.len()
            ),
        })
    } else {
        Err(VerifyError(format!(
            "forge on the {label} tree disagreed with itself: JSON says {} but the exit code is {}. \
             The verdict is not taken from log text.",
            if all_pass { "pass" } else { "fail" },
            if exit_ok { "0" } else { "non-zero" }
        )))
    }
}

/// Test results from forge `--json` stdout. Strings are not scanned, so a
/// log line that contains `"success": false` cannot decide the verdict.
fn parse_results(stdout: &str) -> Result<Vec<bool>, String> {
    let parsed = serde_json::from_str::<Value>(stdout.trim())
        .ok()
        .or_else(|| {
            let start = stdout.find('{')?;
            let mut stream =
                serde_json::Deserializer::from_str(&stdout[start..]).into_iter::<Value>();
            match stream.next() {
                Some(Ok(v)) => Some(v),
                _ => None,
            }
        });
    let Some(value) = parsed else {
        return Err("stdout was not JSON".into());
    };
    let mut out = Vec::new();
    walk_results(&value, &mut out);
    if out.is_empty() {
        Err("JSON contained no test result".into())
    } else {
        Ok(out)
    }
}

fn walk_results(v: &Value, out: &mut Vec<bool>) {
    match v {
        Value::Array(items) => {
            for item in items {
                walk_results(item, out);
            }
        }
        Value::Object(map) => {
            if let Some(status) = map.get("status").and_then(Value::as_str)
                && matches!(status, "Success" | "Failure" | "Pass" | "Fail")
            {
                out.push(status == "Success" || status == "Pass");
                return;
            }
            if let Some(success) = map.get("success").and_then(Value::as_bool) {
                let metadata = map.keys().all(|k| {
                    matches!(
                        k.as_str(),
                        "success"
                            | "reason"
                            | "counterexample"
                            | "logs"
                            | "decoded_logs"
                            | "kind"
                            | "duration"
                            | "traces"
                            | "gas"
                            | "labeled_addresses"
                            | "breakpoints"
                            | "debug"
                    )
                });
                if metadata {
                    out.push(success);
                    return;
                }
            }
            for child in map.values() {
                walk_results(child, out);
            }
        }
        _ => {}
    }
}

/// Reads one Foundry config with the size and type checks. `None` when the
/// file does not exist, which enables nothing.
///
/// The size and type are checked before the read. A config is small; a tree
/// could otherwise ship a huge `foundry.toml` and make this allocate it, and
/// a FIFO named `foundry.toml` would block.
fn read_config(path: &Path) -> Result<Option<toml::Value>, VerifyError> {
    let meta = match std::fs::metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(VerifyError(format!("{}: {e}", path.display()))),
    };
    if !meta.is_file() {
        return Err(VerifyError(format!(
            "{}: not a regular file; verify-fix will not read a Foundry config it cannot bound",
            path.display()
        )));
    }
    if meta.len() > MAX_CONFIG_BYTES {
        return Err(VerifyError(format!(
            "{}: {} bytes is over the {MAX_CONFIG_BYTES}-byte limit for a Foundry config; it was not read",
            path.display(),
            meta.len()
        )));
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| VerifyError(format!("{}: {e}", path.display())))?;
    let doc: toml::Value = toml::from_str(&text).map_err(|e| {
        VerifyError(format!(
            "{}: cannot be parsed as TOML, so verify-fix cannot prove ffi is off and fs_permissions are read-only: {e}",
            path.display()
        ))
    })?;
    Ok(Some(doc))
}

/// Whether `dir/foundry.toml` enables `ffi` anywhere. Parsed as TOML, so
/// spacing, tabs, quoted keys and dotted keys cannot change the answer. Any
/// table or inline table with a key named `ffi` whose value is the boolean
/// `true` counts: the scan is deliberately conservative, so a profile this
/// program has not heard of (or a top-level `ffi`) is refused too.
#[cfg(test)]
fn ffi_enabled(dir: &Path) -> Result<bool, VerifyError> {
    match read_config(&dir.join("foundry.toml"))? {
        Some(doc) => Ok(ffi_set_true(&doc)),
        None => Ok(false),
    }
}

fn ffi_set_true(value: &toml::Value) -> bool {
    match value {
        toml::Value::Table(table) => table.iter().any(|(key, value)| {
            (key == "ffi" && value.as_bool() == Some(true)) || ffi_set_true(value)
        }),
        toml::Value::Array(items) => items.iter().any(ffi_set_true),
        _ => false,
    }
}

/// A problem with an `fs_permissions` entry, or `None` when every entry grants
/// only reading. Any access other than exactly `"read"`, a missing access, a
/// non-array value or a non-table entry is refused. Write access would let the
/// first PoC run rewrite the other tree's or the global config to
/// `ffi = true`, and the second run would then execute `ffi`.
fn fs_permissions_problem(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::Table(table) => {
            for (key, value) in table {
                if key == "fs_permissions" {
                    let Some(entries) = value.as_array() else {
                        return Some("fs_permissions is not an array".to_string());
                    };
                    for entry in entries {
                        let Some(entry) = entry.as_table() else {
                            return Some("an fs_permissions entry is not a table".to_string());
                        };
                        match entry.get("access").and_then(toml::Value::as_str) {
                            Some("read") => {}
                            Some(other) => {
                                return Some(format!("an entry grants access = \"{other}\""));
                            }
                            None => {
                                return Some(
                                    "an entry has no access (or it is not a string)".into(),
                                );
                            }
                        }
                    }
                }
                if let Some(problem) = fs_permissions_problem(value) {
                    return Some(problem);
                }
            }
            None
        }
        toml::Value::Array(items) => items.iter().find_map(fs_permissions_problem),
        _ => None,
    }
}

fn ffi_error(what: &str) -> VerifyError {
    VerifyError(format!(
        "{what} sets ffi = true. verify-fix will not run a project that can \
         execute host commands. Set ffi = false. FOUNDRY_FFI=false is not a control: Foundry \
         documents that variable as an explicit exception to env overrides. A container with \
         ffi off is the supported way to run an untrusted PoC; see docs/verify-fix.md."
    ))
}

fn fs_error(what: &str, problem: &str) -> VerifyError {
    VerifyError(format!(
        "{what} sets fs_permissions with {problem}. verify-fix refuses a Foundry config that grants file \
         write access: the first PoC run could rewrite the other tree's or the global config to \
         ffi = true, and the second run would execute ffi. Only access = \"read\" entries are \
         allowed; see docs/verify-fix.md."
    ))
}

/// Checks one config file for both controls: `ffi` anywhere, and any
/// `fs_permissions` entry that is not read-only.
fn refuse_config_file(path: &Path, what: &str) -> Result<(), VerifyError> {
    let Some(doc) = read_config(path)? else {
        return Ok(());
    };
    if ffi_set_true(&doc) {
        return Err(ffi_error(&format!("{what} ({})", path.display())));
    }
    if let Some(problem) = fs_permissions_problem(&doc) {
        return Err(fs_error(&format!("{what} ({})", path.display()), &problem));
    }
    Ok(())
}

fn refuse_config_dir(dir: &Path, what: &str) -> Result<(), VerifyError> {
    refuse_config_file(
        &dir.join("foundry.toml"),
        &format!("foundry.toml in {what}"),
    )
}

/// Re-reads the configs and dotenv files the next forge run can see. This
/// runs immediately before every forge start: the fs_permissions refusal is
/// the primary control, but this also catches the simple case where the first
/// PoC run rewrites the other copy, or the global config, after the initial
/// check. A lingering process can still write after this check; the container
/// boundary covers that, as docs/verify-fix.md says.
fn revalidate_trees(
    global_config: Option<&Path>,
    copies: &[(&Path, &str)],
) -> Result<(), VerifyError> {
    if let Some(path) = global_config {
        refuse_config_file(path, "Foundry's global config")?;
    }
    for (dir, what) in copies {
        refuse_config_dir(dir, what)?;
        refuse_dotenv(dir, what)?;
    }
    Ok(())
}

/// A `.env` in a tree forge will run in is refused. Foundry loads dotenv
/// after it starts, and a `.env` can set `FOUNDRY_CONFIG` to a config this
/// command never parsed. `copy_tree` skips dotfiles, so the only way one
/// appears in a copy is a `--patch` that creates it; the check runs after the
/// patch and is defense in depth next to the pinned `FOUNDRY_CONFIG`.
fn refuse_dotenv(dir: &Path, what: &str) -> Result<(), VerifyError> {
    let path = dir.join(".env");
    match std::fs::symlink_metadata(&path) {
        Ok(_) => Err(VerifyError(format!(
            "{what} contains a .env file ({}). Foundry loads dotenv after it starts, so a .env can point FOUNDRY_CONFIG at a config verify-fix never checked. verify-fix refuses a .env in the trees it runs; remove it from the patch or the project.",
            path.display()
        ))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(VerifyError(format!("{}: {e}", path.display()))),
    }
}

/// The absolute path of the config the forge child is pinned to. If the tree
/// has no `foundry.toml`, a minimal one is created in the copy. The config
/// has already been checked for `ffi` and `fs_permissions` by
/// `refuse_config_dir`; the absolute path is
/// what stops a `.env` from redirecting `FOUNDRY_CONFIG` (dotenvy does not
/// override an already-set variable).
fn pinned_config(dir: &Path) -> Result<PathBuf, VerifyError> {
    let path = absolute_from_cwd(&dir.join("foundry.toml"))?;
    if !path.is_file() {
        std::fs::write(&path, "[profile.default]\n")
            .map_err(|e| VerifyError(format!("{}: {e}", path.display())))?;
    }
    Ok(path)
}

/// Foundry's global config: `~/.foundry/foundry.toml`, where the home
/// directory is `USERPROFILE` (then `HOME`) on Windows and `HOME` elsewhere.
/// A config in an *ancestor* of the temp copy is not found this way; that
/// residual is documented rather than silently claimed.
fn global_foundry_toml() -> Option<PathBuf> {
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME");
    home.filter(|h| !h.is_empty())
        .map(|h| PathBuf::from(h).join(".foundry").join("foundry.toml"))
}

/// Resolves a path argument from the caller's current directory. The temp
/// copy becomes the forge process's working directory, so a path that is
/// used after that (the patch, or a `--forge` with a separator) has to be
/// fixed before it is reinterpreted.
fn absolute_from_cwd(path: &Path) -> Result<PathBuf, VerifyError> {
    std::path::absolute(path).map_err(|e| VerifyError(format!("{}: {e}", path.display())))
}

/// A bare `forge` keeps PATH lookup; a value containing a path separator is
/// resolved from the caller's current directory, so the temp copy's working
/// directory does not change which binary runs.
fn forge_program(forge: &Path) -> Result<PathBuf, VerifyError> {
    let text = forge.to_string_lossy();
    if text.contains('/') || text.contains('\\') {
        absolute_from_cwd(forge)
    } else {
        Ok(forge.to_path_buf())
    }
}

fn poc_arg(poc: &Path, root: &Path, work: &Path) -> String {
    if poc.is_absolute() {
        poc.strip_prefix(root)
            .or_else(|_| poc.strip_prefix(work))
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| poc.to_string_lossy().replace('\\', "/"))
    } else {
        poc.to_string_lossy().replace('\\', "/")
    }
}

/// The largest patch `verify-fix` reads. A fix is a few kilobytes; the cap
/// keeps a huge file from being read whole into memory.
const MAX_PATCH_BYTES: u64 = 8 * 1024 * 1024;

/// The largest Foundry config (a tree's or the global one) that is read.
const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

/// How long one forge run may take. `check` never runs forge; this bounds the
/// opt-in command when a PoC or a fork hangs.
const FORGE_TIMEOUT: Duration = Duration::from_secs(600);

/// The most a forge run may print. A verdict is never taken from a truncated
/// log, so exceeding either cap is a [`VerifyError`], not a failed PoC.
const FORGE_STDOUT_CAP: usize = 64 * 1024 * 1024;
const FORGE_STDERR_CAP: usize = 1024 * 1024;

fn apply_patch(dir: &Path, patch: &Path) -> Result<(), VerifyError> {
    // Called directly by tests with a path relative to the test process's
    // current directory; `git apply` below runs with the temp copy as cwd.
    let patch = absolute_from_cwd(patch)?;
    let len = std::fs::metadata(&patch)
        .map_err(|e| VerifyError(format!("{}: {e}", patch.display())))?
        .len();
    if len > MAX_PATCH_BYTES {
        return Err(VerifyError(format!(
            "{}: {len} bytes is too large; verify-fix reads a patch of up to {MAX_PATCH_BYTES} bytes",
            patch.display()
        )));
    }
    let text = std::fs::read_to_string(&patch)
        .map_err(|e| VerifyError(format!("{}: {e}", patch.display())))?;
    let paths = patch_paths(&text)?;
    for rel in &paths {
        dest_in(dir, rel)?;
    }
    // git is required. A hand-written applier that accepted a different
    // subset of diffs could leave a tree the patch does not describe, and a
    // verdict on that tree would be meaningless. `--check` first means a
    // patch that cannot apply cleanly is refused before any file changes.
    git_apply(dir, &patch, true)?;
    git_apply(dir, &patch, false)
}

fn git_apply(dir: &Path, patch: &Path, check: bool) -> Result<(), VerifyError> {
    let mut cmd = Command::new("git");
    cmd.arg("apply");
    if check {
        cmd.arg("--check");
    }
    cmd.arg("--whitespace=nowarn").arg(patch).current_dir(dir);
    let out = cmd.output().map_err(|e| {
        VerifyError(format!(
            "could not run git to apply the patch: {e}. verify-fix needs git for --patch; it \
             does not fall back to a hand-written applier."
        ))
    })?;
    if out.status.success() {
        return Ok(());
    }
    Err(VerifyError(format!(
        "git apply{} refused the patch, so the patched tree would not be the one the patch \
         describes:\n{}",
        if check { " --check" } else { "" },
        first_lines(
            &format!(
                "{}\n{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ),
            20
        )
    )))
}

/// Paths a unified diff would write. Absolute paths and `..` are rejected
/// before any apply, so the patch cannot leave the temp tree.
fn patch_paths(text: &str) -> Result<Vec<String>, VerifyError> {
    let mut paths = Vec::new();
    for line in text.lines() {
        if is_symlink_header(line) {
            return Err(VerifyError(
                "the patch creates or changes a symbolic link (mode 120000). A link can point outside the temp tree, so verify-fix refuses it."
                    .into(),
            ));
        }
        let raw = if let Some(rest) = line.strip_prefix("diff --git ") {
            for part in rest.split_whitespace() {
                let p = strip_ab(part);
                if p != "/dev/null" {
                    paths.push(p);
                }
            }
            continue;
        } else if let Some(rest) = line
            .strip_prefix("--- ")
            .or_else(|| line.strip_prefix("+++ "))
        {
            rest.split_whitespace().next().unwrap_or("")
        } else if let Some(rest) = line
            .strip_prefix("rename from ")
            .or_else(|| line.strip_prefix("rename to "))
        {
            rest.trim()
        } else {
            continue;
        };
        let p = strip_ab(raw);
        if p != "/dev/null" && !p.is_empty() {
            paths.push(p);
        }
    }
    if paths.is_empty() {
        return Err(VerifyError(
            "the patch names no files. verify-fix applies a unified diff inside the temp tree only.".into(),
        ));
    }
    for p in &paths {
        if !path_is_confined(p) {
            return Err(VerifyError(format!(
                "refusing patch path `{p}`: it is absolute or contains `..`. The patch stays inside the temp tree."
            )));
        }
    }
    Ok(paths)
}

/// A git header that sets a symbolic-link mode: `new file mode 120000`,
/// `new mode 120000`, `old mode 120000`, or an `index` line that ends in it.
fn is_symlink_header(line: &str) -> bool {
    let t = line.trim_end();
    let mode_line =
        t.starts_with("new file mode ") || t.starts_with("new mode ") || t.starts_with("old mode ");
    (mode_line || t.starts_with("index ")) && t.ends_with(" 120000")
}

fn strip_ab(p: &str) -> String {
    let p = p.trim().trim_matches('"');
    p.strip_prefix("a/")
        .or_else(|| p.strip_prefix("b/"))
        .unwrap_or(p)
        .to_string()
}

fn path_is_confined(p: &str) -> bool {
    if p.starts_with('/') || p.starts_with('\\') {
        return false;
    }
    if p.chars().nth(1) == Some(':') {
        return false;
    }
    !p.split(['/', '\\']).any(|c| c.is_empty() || c == "..")
}

fn dest_in(dir: &Path, rel: &str) -> Result<PathBuf, VerifyError> {
    if !path_is_confined(rel) {
        return Err(VerifyError(format!("patch path `{rel}` escapes the tree")));
    }
    let dest = dir.join(rel);
    let dir_abs = std::path::absolute(dir).map_err(|e| VerifyError(e.to_string()))?;
    let dest_abs = std::path::absolute(&dest).unwrap_or(dest.clone());
    if !dest_abs.starts_with(&dir_abs) {
        return Err(VerifyError(format!(
            "patch path `{rel}` resolves outside the temp tree"
        )));
    }
    Ok(dest)
}

/// A fresh directory for one copy of a project tree. The name is unique per
/// process (the counter) and per process ID, and `create_dir` refuses an
/// existing directory, so two copies can never share a tree. A shared tree is
/// what let one test remove the working directory another test's forge was
/// running in.
fn copy_of(from: &Path) -> Result<PathBuf, VerifyError> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let to = loop {
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let candidate =
            std::env::temp_dir().join(format!("realism-gate-verify-{}-{seq}", std::process::id()));
        match std::fs::create_dir(&candidate) {
            Ok(()) => break candidate,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(VerifyError(format!("{}: {e}", candidate.display()))),
        }
    };
    if let Err(e) = copy_tree(from, &to) {
        let _ = std::fs::remove_dir_all(&to);
        return Err(e);
    }
    Ok(to)
}

#[derive(Default)]
struct TempDirs(Vec<PathBuf>);

impl TempDirs {
    fn push(&mut self, path: PathBuf) -> PathBuf {
        self.0.push(path.clone());
        path
    }
}

impl Drop for TempDirs {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

/// The most files, bytes, directories and directory levels a copied tree may
/// contain. The project and `--patched` tree are attacker-controlled: without
/// caps, a hostile repository can fill the disk with files or millions of
/// empty directories before forge's own limits apply.
const MAX_COPY_FILES: usize = 50_000;
const MAX_COPY_BYTES: u64 = 512 * 1024 * 1024;
const MAX_COPY_DIRS: usize = 50_000;
const MAX_COPY_DEPTH: usize = 64;

fn copy_tree(from: &Path, to: &Path) -> Result<(), VerifyError> {
    copy_tree_with_limits(
        from,
        to,
        MAX_COPY_FILES,
        MAX_COPY_BYTES,
        MAX_COPY_DIRS,
        MAX_COPY_DEPTH,
    )
}

/// The body of [`copy_tree`] with injectable limits, so the caps can be tested
/// without generating 50 000 files or directories.
fn copy_tree_with_limits(
    from: &Path,
    to: &Path,
    max_files: usize,
    max_bytes: u64,
    max_dirs: usize,
    max_depth: usize,
) -> Result<(), VerifyError> {
    std::fs::create_dir_all(to).map_err(|e| VerifyError(format!("{}: {e}", to.display())))?;
    let skip = [
        "out",
        "cache",
        "broadcast",
        "target",
        "node_modules",
        ".git",
        "artifacts",
    ];
    struct Budget {
        files: usize,
        bytes: u64,
        dirs: usize,
    }
    fn walk(
        from: &Path,
        to: &Path,
        skip: &[&str],
        depth: usize,
        limits: (usize, u64, usize, usize),
        budget: &mut Budget,
    ) -> Result<(), VerifyError> {
        let (max_files, max_bytes, max_dirs, max_depth) = limits;
        if depth > max_depth {
            return Err(VerifyError(format!(
                "{}: directory nesting is deeper than the {max_depth}-level limit; verify-fix will not copy it",
                from.display()
            )));
        }
        for entry in
            std::fs::read_dir(from).map_err(|e| VerifyError(format!("{}: {e}", from.display())))?
        {
            let entry = entry.map_err(|e| VerifyError(e.to_string()))?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if skip.contains(&name.as_ref()) || name.starts_with('.') && name != ".gitignore" {
                continue;
            }
            let dest = to.join(entry.file_name());
            let ty = entry.file_type().map_err(|e| VerifyError(e.to_string()))?;
            if ty.is_dir() {
                budget.dirs += 1;
                if budget.dirs > max_dirs {
                    return Err(VerifyError(format!(
                        "the tree being copied has more than {max_dirs} directories; verify-fix will not copy it"
                    )));
                }
                std::fs::create_dir_all(&dest)
                    .map_err(|e| VerifyError(format!("{}: {e}", dest.display())))?;
                walk(&entry.path(), &dest, skip, depth + 1, limits, budget)?;
            } else if ty.is_file() {
                let len = entry
                    .metadata()
                    .map_err(|e| VerifyError(format!("{}: {e}", entry.path().display())))?
                    .len();
                budget.files += 1;
                if budget.files > max_files {
                    return Err(VerifyError(format!(
                        "the tree being copied has more than {max_files} files; verify-fix will not copy it"
                    )));
                }
                budget.bytes = budget.bytes.saturating_add(len);
                if budget.bytes > max_bytes {
                    return Err(VerifyError(format!(
                        "the tree being copied is larger than {max_bytes} bytes; verify-fix will not copy it"
                    )));
                }
                std::fs::copy(entry.path(), &dest)
                    .map_err(|e| VerifyError(format!("{}: {e}", dest.display())))?;
            }
        }
        Ok(())
    }
    let mut budget = Budget {
        files: 0,
        bytes: 0,
        dirs: 0,
    };
    walk(
        from,
        to,
        &skip,
        1,
        (max_files, max_bytes, max_dirs, max_depth),
        &mut budget,
    )
}

fn first_lines(text: &str, n: usize) -> String {
    text.lines().take(n).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        // Write to a temporary name and rename into place. On Linux, exec'ing a
        // file that is still open for writing fails with ETXTBSY (os error 26),
        // and a fork from another test thread can hold that write handle for an
        // instant. Nothing ever executes the temporary name.
        let tmp = dir.join(format!("{name}.tmp"));
        std::fs::write(&tmp, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&tmp).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&tmp, perms).unwrap();
        }
        std::fs::rename(&tmp, &path).unwrap();
        #[cfg(unix)]
        {
            path
        }
        // Windows cannot execute a shell script (error 193). The stand-in is
        // a .bat that runs the same script under bash, which the runner has.
        // The gate still launches a normal forge binary directly.
        #[cfg(windows)]
        {
            let bat = dir.join(format!("{name}.bat"));
            let script_name = path.file_name().unwrap().to_string_lossy();
            // Neither bash on PATH is guaranteed to read a Windows path: on a
            // machine with WSL, `bash` is the Linux one and `C:/` does not
            // exist there. Git for Windows' bash reads a forward-slash path,
            // so it is preferred when installed, and the wrapper rewrites its
            // own directory to forward slashes. The working directory the gate
            // set is kept: a script may depend on `$PWD`.
            let bash = [
                "C:\\Program Files\\Git\\bin\\bash.exe",
                "C:\\Program Files\\Git\\usr\\bin\\bash.exe",
            ]
            .into_iter()
            .find(|p| Path::new(p).is_file())
            .unwrap_or("bash");
            std::fs::write(
                &bat,
                format!(
                    "@echo off\r\nset \"d=%~dp0\"\r\nset \"d=%d:\\=/%\"\r\n\"{bash}\" \"%d%{script_name}\" %*\r\n"
                ),
            )
            .unwrap();
            bat
        }
    }

    fn req(
        dir: &Path,
        forge: PathBuf,
        poc_dir: Option<PathBuf>,
        patch: Option<PathBuf>,
    ) -> Request {
        Request {
            poc: PathBuf::from("test/Poc.t.sol"),
            patch,
            patched_dir: poc_dir,
            match_test: None,
            forge,
            root: dir.to_path_buf(),
            host: Some(HostPolicy::Sandbox),
        }
    }

    /// The unit tests pass no global config, so a developer's own
    /// `~/.foundry/foundry.toml` cannot change their result. The integration
    /// tests cover the public entry point and the global config.
    fn run(req: &Request) -> Result<Outcome, VerifyError> {
        verify_with_global_config(req, None)
    }

    #[test]
    fn a_poc_that_fails_on_both_sides_is_not_a_fix() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let forge = script(
            &dir,
            "forge",
            "#!/bin/sh\nprintf '%s\\n' '{\"suite\":{\"test_a\":{\"success\":false}}}'\nexit 1\n",
        );
        let poc_dir = dir.join("patched");
        std::fs::create_dir_all(&poc_dir).unwrap();
        let out = run(&req(&dir, forge, Some(poc_dir), None)).unwrap();
        assert!(
            matches!(out, Outcome::BugDoesNotReproduce { .. }),
            "{out:?}"
        );
    }

    #[test]
    fn a_passing_poc_means_the_fix_does_not_hold() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let forge = script(
            &dir,
            "forge",
            "#!/bin/sh\nprintf '%s\\n' '{\"suite\":{\"test_a\":{\"success\": true, \"logs\":[\"\\\"success\\\": false\"]}}}'\nexit 0\n",
        );
        let poc_dir = dir.join("patched");
        std::fs::create_dir_all(&poc_dir).unwrap();
        let out = run(&req(&dir, forge, Some(poc_dir), None)).unwrap();
        assert!(matches!(out, Outcome::FixDoesNotHold { .. }), "{out:?}");
    }

    #[test]
    fn a_log_line_that_says_success_false_does_not_prove_a_fix() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        // The old parser treated this stdout as a failure and reported FixHolds.
        let forge = script(
            &dir,
            "forge",
            "#!/bin/sh\nprintf '%s\\n' '\"success\": false' '{\"t\":{\"test_a\":{\"success\":true}}}'\nexit 0\n",
        );
        let poc_dir = dir.join("patched");
        std::fs::create_dir_all(&poc_dir).unwrap();
        let out = run(&req(&dir, forge, Some(poc_dir), None)).unwrap();
        assert!(matches!(out, Outcome::FixDoesNotHold { .. }), "{out:?}");
    }

    #[test]
    fn a_compile_error_is_not_a_failed_poc() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let forge = script(
            &dir,
            "forge",
            "#!/bin/sh\necho 'Compiler run failed' >&2\nexit 1\n",
        );
        let poc_dir = dir.join("patched");
        std::fs::create_dir_all(&poc_dir).unwrap();
        let err = run(&req(&dir, forge, Some(poc_dir), None)).unwrap_err();
        assert!(err.0.contains("compile"), "{}", err.0);
    }

    #[test]
    fn a_patch_is_applied_before_forge_runs_and_the_baseline_must_pass() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/A.sol"), "contract A { uint x = 1; }\n").unwrap();
        let patch = dir.join("fix.diff");
        std::fs::write(
            &patch,
            "--- a/src/A.sol\n+++ b/src/A.sol\n@@ -1 +1 @@\n-contract A { uint x = 1; }\n+contract A { uint x = 2; }\n",
        )
        .unwrap();
        let forge = script(
            &dir,
            "forge",
            "#!/bin/sh\nif grep -q 'x = 2' \"$PWD/src/A.sol\"; then\n  printf '%s\\n' '{\"t\":{\"test_a\":{\"success\":false}}}'\n  exit 1\nfi\nprintf '%s\\n' '{\"t\":{\"test_a\":{\"success\":true}}}'\nexit 0\n",
        );
        let out = run(&Request {
            poc: PathBuf::from("test/Poc.t.sol"),
            patch: Some(patch),
            patched_dir: None,
            match_test: Some("test_a".into()),
            forge,
            root: dir.clone(),
            host: Some(HostPolicy::Sandbox),
        })
        .unwrap();
        assert!(matches!(out, Outcome::FixHolds { .. }), "{out:?}");
        let original = std::fs::read_to_string(dir.join("src/A.sol")).unwrap();
        assert!(original.contains("x = 1"), "{original}");
    }

    #[test]
    fn a_patch_that_escapes_the_tree_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/A.sol"), "contract A { uint x = 1; }\n").unwrap();
        let outside = dir.join("outside.txt");
        let patch = dir.join("escape.diff");
        std::fs::write(
            &patch,
            "--- a/../outside.txt\n+++ b/../outside.txt\n@@ -0,0 +1 @@\n+pwned\n",
        )
        .unwrap();
        let forge = script(&dir, "forge", "#!/bin/sh\nexit 0\n");
        let err = run(&Request {
            poc: PathBuf::from("test/Poc.t.sol"),
            patch: Some(patch),
            patched_dir: None,
            match_test: None,
            forge,
            root: dir.clone(),
            host: Some(HostPolicy::Sandbox),
        })
        .unwrap_err();
        assert!(
            err.0.contains("escapes") || err.0.contains(".."),
            "{}",
            err.0
        );
        assert!(!outside.exists(), "the patch wrote outside the temp tree");
    }

    #[test]
    fn ffi_in_foundry_toml_refuses_to_run() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        std::fs::write(dir.join("foundry.toml"), "[profile.default]\nffi = true\n").unwrap();
        let forge = script(&dir, "forge", "#!/bin/sh\nexit 0\n");
        let poc_dir = dir.join("patched");
        std::fs::create_dir_all(&poc_dir).unwrap();
        let err = run(&req(&dir, forge, Some(poc_dir), None)).unwrap_err();
        assert!(err.0.contains("ffi"), "{}", err.0);
    }

    /// Spellings a TOML parser accepts for the same setting. The old
    /// comment-stripped heuristic missed the ones with tabs, quotes and
    /// dotted keys; a real parse cannot.
    const FFI_TRUE: &[&str] = &[
        "[profile.default]\nffi = true\n",
        "[profile.default]\nffi\t=\ttrue\n",
        "[profile.default]\nffi=true\n",
        "[profile.default]\n\"ffi\" = true\n",
        "[profile.default]\n'ffi' = true\n",
        "[profile.default]\nffi = true # enabled\n",
        "profile.default.ffi = true\n",
        "ffi = true\n",
        "[profile.ci]\nffi = true\n",
        "profile = { default = { ffi = true } }\n",
    ];

    #[test]
    fn ffi_is_seen_however_toml_spells_it() {
        for text in FFI_TRUE {
            let tmp = tempfile::tempdir().unwrap();
            std::fs::write(tmp.path().join("foundry.toml"), text).unwrap();
            assert!(
                ffi_enabled(tmp.path()).unwrap(),
                "{text:?} was not read as ffi = true"
            );
        }
    }

    #[test]
    fn a_setting_that_does_not_enable_ffi_is_not_refused() {
        for text in [
            "[profile.default]\nffi = false\n",
            "[profile.default]\nffi = \"true\"\n",
            "[profile.default]\nffi_mode = true\n",
            "[profile.default]\nremeas = true\n",
            "# [profile.default]\n# ffi = true\n",
        ] {
            let tmp = tempfile::tempdir().unwrap();
            std::fs::write(tmp.path().join("foundry.toml"), text).unwrap();
            assert!(
                !ffi_enabled(tmp.path()).unwrap(),
                "{text:?} was wrongly read as ffi = true"
            );
        }
    }

    #[test]
    fn a_foundry_toml_that_does_not_parse_is_refused_not_treated_as_off() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("foundry.toml"),
            "[profile.default\nffi = true\n",
        )
        .unwrap();
        let err = ffi_enabled(tmp.path()).unwrap_err();
        assert!(err.0.contains("TOML"), "{}", err.0);
    }

    /// A forge stand-in that records that it ran, so a test can prove a
    /// refusal happened before any process was started.
    fn forge_marker(dir: &Path) -> (PathBuf, PathBuf) {
        let marker = dir.join("forge-ran.txt");
        let marker_text = marker.to_string_lossy().replace('\\', "/");
        let body = format!(
            "#!/bin/sh\nprintf 'ran\\n' > '{marker_text}'\nprintf '%s\\n' '{{\"t\":{{\"test_a\":{{\"success\":true}}}}}}'\nexit 0\n"
        );
        (script(dir, "forge-marker", &body), marker)
    }

    #[test]
    fn no_attestation_is_refused_before_forge_is_started() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let (forge, marker) = forge_marker(&dir);
        let poc_dir = dir.join("patched");
        std::fs::create_dir_all(&poc_dir).unwrap();
        let mut request = req(&dir, forge, Some(poc_dir), None);
        request.host = None;
        let err = run(&request).unwrap_err();
        assert!(
            err.0
                .contains("will not run an untrusted forge project on the host"),
            "{}",
            err.0
        );
        assert!(!marker.exists(), "forge was started without an attestation");
    }

    #[test]
    fn a_global_config_with_ffi_refuses_before_forge_is_started() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let (forge, marker) = forge_marker(&dir);
        let global = dir.join("global-foundry.toml");
        std::fs::write(&global, "[profile.default]\nffi = true\n").unwrap();
        let poc_dir = dir.join("patched");
        std::fs::create_dir_all(&poc_dir).unwrap();
        let request = req(&dir, forge, Some(poc_dir), None);
        let err = verify_with_global_config(&request, Some(&global)).unwrap_err();
        assert!(
            err.0.contains("global config") && err.0.contains("ffi"),
            "{}",
            err.0
        );
        assert!(!marker.exists(), "forge ran despite the global ffi config");
    }

    #[test]
    fn a_bare_forge_name_keeps_path_lookup() {
        assert_eq!(
            forge_program(Path::new("forge")).unwrap(),
            PathBuf::from("forge")
        );
    }

    #[test]
    fn a_forge_path_with_a_separator_is_resolved_from_the_process_cwd() {
        let resolved = forge_program(Path::new("../tools/forge")).unwrap();
        assert!(resolved.is_absolute(), "{}", resolved.display());
        assert!(
            resolved.ends_with(Path::new("tools").join("forge")),
            "{}",
            resolved.display()
        );
    }

    /// Runs one process through the bounded waiter with test-sized limits.
    fn run_process(
        forge: &Path,
        timeout: Duration,
        stdout_cap: usize,
        stderr_cap: usize,
    ) -> Result<(), String> {
        let mut cmd = forge_command(forge);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = spawn_with_retry(&mut cmd).map_err(|e| e.to_string())?;
        wait_forge(&mut child, timeout, stdout_cap, stderr_cap)
            .map(|_| ())
            .map_err(|e| e.0)
    }

    #[test]
    fn a_forge_that_runs_past_the_timeout_is_killed_and_is_not_a_verdict() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let forge = script(&dir, "forge-sleep", "#!/bin/sh\nsleep 30\nexit 0\n");
        let start = Instant::now();
        let err = run_process(&forge, Duration::from_millis(300), 1 << 20, 1 << 20).unwrap_err();
        assert!(err.contains("did not finish"), "{err}");
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "the timeout did not bound the run: {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn a_forge_that_floods_stdout_is_killed_and_is_not_a_verdict() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let forge = script(
            &dir,
            "forge-flood",
            "#!/bin/sh\nwhile :; do printf 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'; done\n",
        );
        let start = Instant::now();
        let err = run_process(&forge, Duration::from_secs(30), 1024, 1024).unwrap_err();
        assert!(err.contains("stdout") && err.contains("1024"), "{err}");
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "the flood cap did not bound the run: {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn a_forge_that_floods_and_exits_at_once_is_still_not_a_verdict() {
        // The child can exit between the last poll and `try_wait`; the
        // post-wait overflow check must still refuse the truncated stream.
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let forge = script(
            &dir,
            "forge-flood-exit",
            "#!/bin/sh\ni=0\nwhile [ $i -lt 100 ]; do printf 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'; i=$((i+1)); done\nexit 0\n",
        );
        let err = run_process(&forge, Duration::from_secs(30), 1024, 1024).unwrap_err();
        assert!(err.contains("stdout") && err.contains("1024"), "{err}");
    }

    #[test]
    fn an_oversized_foundry_toml_is_refused_before_it_is_read() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let file = std::fs::File::create(dir.join("foundry.toml")).unwrap();
        file.set_len(MAX_CONFIG_BYTES + 1).unwrap();
        drop(file);
        let err = ffi_enabled(&dir).unwrap_err();
        assert!(
            err.0.contains(&MAX_CONFIG_BYTES.to_string()) && err.0.contains("not read"),
            "{}",
            err.0
        );
    }

    #[test]
    fn fs_permissions_must_be_read_only() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("foundry.toml");
        for (text, needle) in [
            (
                "[profile.default]\nfs_permissions = [{ access = \"write\", path = \".\" }]\n",
                "write",
            ),
            (
                "[profile.default]\nfs_permissions = [{ access = \"read-write\", path = \".\" }]\n",
                "read-write",
            ),
            (
                "[profile.default]\nfs_permissions = [{ path = \".\" }]\n",
                "no access",
            ),
            (
                "[profile.default]\nfs_permissions = \"read\"\n",
                "not an array",
            ),
        ] {
            std::fs::write(&path, text).unwrap();
            let err = refuse_config_file(&path, "test config").unwrap_err();
            assert!(
                err.0.contains("fs_permissions") && err.0.contains(needle),
                "{text:?}: {}",
                err.0
            );
        }
        for text in [
            "[profile.default]\nffi = false\n",
            "[profile.default]\nfs_permissions = []\n",
            "[profile.default]\nfs_permissions = [{ access = \"read\", path = \".\" }]\n",
        ] {
            std::fs::write(&path, text).unwrap();
            refuse_config_file(&path, "test config").unwrap();
        }
    }

    #[test]
    fn a_rewritten_tree_or_global_config_is_refused_before_the_next_run() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let benign = "[profile.default]\nffi = false\n";
        std::fs::write(a.join("foundry.toml"), benign).unwrap();
        std::fs::write(b.join("foundry.toml"), benign).unwrap();
        let global = tmp.path().join("global.toml");
        std::fs::write(&global, benign).unwrap();
        revalidate_trees(Some(&global), &[(&a, "a"), (&b, "b")]).unwrap();

        std::fs::write(b.join("foundry.toml"), "[profile.default]\nffi = true\n").unwrap();
        let err = revalidate_trees(Some(&global), &[(&a, "a"), (&b, "b")]).unwrap_err();
        assert!(err.0.contains("ffi"), "{}", err.0);

        std::fs::write(
            b.join("foundry.toml"),
            "[profile.default]\nfs_permissions = [{ access = \"write\", path = \".\" }]\n",
        )
        .unwrap();
        let err = revalidate_trees(Some(&global), &[(&a, "a"), (&b, "b")]).unwrap_err();
        assert!(err.0.contains("fs_permissions"), "{}", err.0);

        std::fs::write(b.join("foundry.toml"), benign).unwrap();
        std::fs::write(b.join(".env"), "FOUNDRY_CONFIG=evil.toml\n").unwrap();
        let err = revalidate_trees(Some(&global), &[(&a, "a"), (&b, "b")]).unwrap_err();
        assert!(err.0.contains(".env"), "{}", err.0);
        std::fs::remove_file(b.join(".env")).unwrap();

        std::fs::write(&global, "[profile.default]\nffi = true\n").unwrap();
        let err = revalidate_trees(Some(&global), &[(&a, "a"), (&b, "b")]).unwrap_err();
        assert!(err.0.contains("global config"), "{}", err.0);
    }

    #[test]
    fn the_copy_file_byte_and_directory_caps_are_enforced() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();

        let files = base.join("files");
        std::fs::create_dir_all(&files).unwrap();
        for name in ["a", "b", "c"] {
            std::fs::write(files.join(name), "x").unwrap();
        }
        let err =
            copy_tree_with_limits(&files, &base.join("files-out"), 2, 1 << 20, 10, 64).unwrap_err();
        assert!(err.0.contains("2 files"), "{}", err.0);

        let bytes = base.join("bytes");
        std::fs::create_dir_all(&bytes).unwrap();
        std::fs::write(bytes.join("a"), "0123456789").unwrap();
        std::fs::write(bytes.join("b"), "0123456789").unwrap();
        let err =
            copy_tree_with_limits(&bytes, &base.join("bytes-out"), 10, 15, 10, 64).unwrap_err();
        assert!(err.0.contains("15 bytes"), "{}", err.0);

        let dirs = base.join("dirs");
        std::fs::create_dir_all(dirs.join("a/b/c")).unwrap();
        let err =
            copy_tree_with_limits(&dirs, &base.join("dirs-out"), 10, 1 << 20, 2, 64).unwrap_err();
        assert!(err.0.contains("2 directories"), "{}", err.0);

        let depth = base.join("depth");
        std::fs::create_dir_all(depth.join("a/b")).unwrap();
        let err =
            copy_tree_with_limits(&depth, &base.join("depth-out"), 10, 1 << 20, 10, 1).unwrap_err();
        assert!(err.0.contains("1-level limit"), "{}", err.0);
    }

    #[test]
    fn status_success_is_parsed_without_reading_log_strings() {
        let v = parse_results(
            r#"{"suite":{"test_a":{"status":"Success","logs":["\"success\": false"]}}}"#,
        )
        .unwrap();
        assert_eq!(v, vec![true]);
        let v = parse_results(r#"{"suite":{"test_a":{"status":"Failure"}}}"#).unwrap();
        assert_eq!(v, vec![false]);
    }

    // ---- Patch and copy escapes beyond the `../` case (threat model H4, H6).

    /// A unified diff whose only header is `+++ TARGET`, which is all that
    /// `patch_paths` reads.
    fn header_only(target: &str) -> String {
        format!("--- a/x\n+++ {target}\n@@ -0,0 +1 @@\n+x\n")
    }

    #[test]
    fn a_patch_path_that_leaves_the_tree_is_refused_however_it_is_spelled() {
        // Each spelling names a file outside the temp tree on some system:
        // absolute, drive-letter (absolute and drive-relative), the Windows
        // verbatim prefix, a UNC share, and `..` written with backslashes.
        for target in [
            "b//abs/evil",
            "b/C:/evil.txt",
            "b/C:evil.txt",
            "b/\\\\?\\C:\\evil.txt",
            "b/\\\\server\\share\\evil.txt",
            "b/..\\evil.txt",
            "b/src\\..\\..\\evil.txt",
            "b/src/../../evil.txt",
        ] {
            let err = patch_paths(&header_only(target));
            assert!(err.is_err(), "`{target}` was accepted: {err:?}");
        }
    }

    #[test]
    fn a_patch_that_creates_a_symlink_is_refused() {
        // `git apply` creates a symlink from a mode 120000 entry, and the
        // symlink's target is not checked. A later read or write through it
        // would leave the tree. Refused before any apply.
        let text = "diff --git a/src/leak.sol b/src/leak.sol\nnew file mode 120000\nindex 0000000..1111111\n--- /dev/null\n+++ b/src/leak.sol\n@@ -0,0 +1 @@\n+/etc/passwd\n";
        let err = patch_paths(text);
        assert!(err.is_err(), "a symlink entry was accepted: {err:?}");
        let text = "diff --git a/src/a.sol b/src/a.sol\nold mode 100644\nnew mode 120000\n--- a/src/a.sol\n+++ b/src/a.sol\n@@ -1 +1 @@\n-x\n+y\n";
        assert!(
            patch_paths(text).is_err(),
            "a mode change to a symlink was accepted"
        );
    }

    #[test]
    fn a_symlink_in_the_project_is_neither_copied_nor_written_through() {
        // The project under review contains a file link and a directory link
        // to a place outside the tree. A copy must not contain either, and a
        // patch aimed at the file link must not write outside the copy.
        let tmp = tempfile::tempdir().unwrap();
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), "untouched\n").unwrap();
        let root = tmp.path().join("project");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/A.sol"), "contract A { uint x = 1; }\n").unwrap();
        if !make_symlink(&outside.join("secret.txt"), &root.join("src/link.sol"))
            || !make_dir_symlink(&outside, &root.join("linkdir"))
        {
            eprintln!("symlinks cannot be created here; the symlink test is skipped");
            return;
        }
        let copy = copy_of(&root).unwrap();
        assert!(
            !copy.join("src/link.sol").exists(),
            "a file link was copied"
        );
        assert!(
            !copy.join("linkdir").exists(),
            "a directory link was copied"
        );

        let patch = tmp.path().join("write-through.diff");
        std::fs::write(
            &patch,
            "--- a/src/link.sol\n+++ b/src/link.sol\n@@ -0,0 +1 @@\n+pwned\n",
        )
        .unwrap();
        let _ = apply_patch(&copy, &patch);
        assert_eq!(
            std::fs::read_to_string(outside.join("secret.txt")).unwrap(),
            "untouched\n",
            "the patch wrote through a link to outside the tree"
        );
        let _ = std::fs::remove_dir_all(&copy);
    }

    #[test]
    fn an_oversized_patch_is_refused_with_a_clear_error() {
        let tmp = tempfile::tempdir().unwrap();
        let patch = tmp.path().join("huge.diff");
        let file = std::fs::File::create(&patch).unwrap();
        file.set_len(MAX_PATCH_BYTES + 1).unwrap();
        drop(file);
        let err = apply_patch(tmp.path(), &patch).unwrap_err();
        assert!(err.0.contains("too large"), "{}", err.0);
    }

    #[cfg(unix)]
    fn make_symlink(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn make_symlink(target: &Path, link: &Path) -> bool {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }

    #[cfg(unix)]
    fn make_dir_symlink(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn make_dir_symlink(target: &Path, link: &Path) -> bool {
        std::os::windows::fs::symlink_dir(target, link).is_ok()
    }
}
