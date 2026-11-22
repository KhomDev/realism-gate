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
//! enable `ffi` and it can make forge open a connection. This command refuses
//! to run when any `foundry.toml` in the tree sets `ffi = true`: the root
//! project, the `--patched` tree, and the copied tree after a `--patch` is
//! applied are all parsed as TOML. The environment variable `FOUNDRY_FFI` is
//! not a control: Foundry documents it as an explicit exception to env
//! overrides, so this command does not rely on setting it.
//!
//! The host decision is part of this function, not only of the CLI: a
//! [`Request`] with no [`HostPolicy`] is refused before a copy is made or a
//! forge process is started, so a library caller cannot skip the check. See
//! `docs/verify-fix.md`.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

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
    // Every tree forge could run in is checked for ffi. The check on the
    // copied tree after `--patch` is applied is the one that covers a patch
    // that adds `ffi = true`.
    refuse_ffi(&req.root, "the project in --root")?;

    let mut temps = TempDirs::default();
    let baseline = temps.push(copy_of(&req.root)?);
    refuse_ffi(&baseline, "the copied project")?;
    let patched = if let Some(dir) = &req.patched_dir {
        if !dir.is_dir() {
            return Err(VerifyError(format!("{}: not a directory", dir.display())));
        }
        refuse_ffi(dir, "the --patched tree")?;
        let copy = temps.push(copy_of(dir)?);
        refuse_ffi(&copy, "the copied --patched tree")?;
        copy
    } else {
        let patch = req.patch.as_ref().expect("patch or dir");
        if !patch.is_file() {
            return Err(VerifyError(format!("{}: no such patch", patch.display())));
        }
        let work = temps.push(copy_of(&req.root)?);
        apply_patch(&work, patch)?;
        refuse_ffi(&work, "the patched copy")?;
        work
    };

    let before = run_forge(req, &baseline, "unpatched")?;
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
fn output_with_retry(cmd: &mut Command) -> std::io::Result<std::process::Output> {
    let mut last = None;
    for _ in 0..10 {
        match cmd.output() {
            Ok(out) => return Ok(out),
            Err(e) if matches!(e.raw_os_error(), Some(26) | Some(32)) => {
                std::thread::sleep(std::time::Duration::from_millis(20));
                last = Some(e);
            }
            Err(e) => return Err(e),
        }
    }
    Err(last.expect("a retryable error was recorded"))
}

#[cfg(not(test))]
fn output_with_retry(cmd: &mut Command) -> std::io::Result<std::process::Output> {
    cmd.output()
}

fn run_forge(req: &Request, work: &Path, label: &str) -> Result<Side, VerifyError> {
    let poc = poc_arg(&req.poc, &req.root, work);
    let mut cmd = forge_command(&req.forge);
    cmd.current_dir(work)
        .arg("test")
        .arg("--match-path")
        .arg(&poc)
        .arg("--json")
        // Ask forge not to honor a config that turns ffi on, and do not add
        // an RPC URL. A fork test still uses whatever URL the test names.
        .env("FOUNDRY_FFI", "false");
    if let Some(name) = &req.match_test {
        cmd.arg("--match-test").arg(name);
    }
    let output = output_with_retry(&mut cmd).map_err(|e| {
        VerifyError(format!(
            "could not run {}: {e}. verify-fix needs forge on PATH, or --forge.",
            req.forge.display()
        ))
    })?;
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

/// Whether `dir/foundry.toml` enables `ffi` anywhere. Parsed as TOML, so
/// spacing, tabs, quoted keys and dotted keys cannot change the answer. Any
/// table or inline table with a key named `ffi` whose value is the boolean
/// `true` counts: the scan is deliberately conservative, so a profile this
/// program has not heard of (or a top-level `ffi`) is refused too.
fn ffi_enabled(dir: &Path) -> Result<bool, VerifyError> {
    let path = dir.join("foundry.toml");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(VerifyError(format!("{}: {e}", path.display()))),
    };
    let doc: toml::Value = toml::from_str(&text).map_err(|e| {
        VerifyError(format!(
            "{}: cannot be parsed as TOML, so verify-fix cannot prove ffi is off: {e}",
            path.display()
        ))
    })?;
    Ok(ffi_set_true(&doc))
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

fn refuse_ffi(dir: &Path, what: &str) -> Result<(), VerifyError> {
    if ffi_enabled(dir)? {
        return Err(VerifyError(format!(
            "foundry.toml in {what} sets ffi = true. verify-fix will not run a project that can \
             execute host commands. Set ffi = false. FOUNDRY_FFI=false is not a control: Foundry \
             documents that variable as an explicit exception to env overrides. A container with \
             ffi off is the supported way to run an untrusted PoC; see docs/verify-fix.md."
        )));
    }
    Ok(())
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

fn apply_patch(dir: &Path, patch: &Path) -> Result<(), VerifyError> {
    let len = std::fs::metadata(patch)
        .map_err(|e| VerifyError(format!("{}: {e}", patch.display())))?
        .len();
    if len > MAX_PATCH_BYTES {
        return Err(VerifyError(format!(
            "{}: {len} bytes is too large; verify-fix reads a patch of up to {MAX_PATCH_BYTES} bytes",
            patch.display()
        )));
    }
    let text = std::fs::read_to_string(patch)
        .map_err(|e| VerifyError(format!("{}: {e}", patch.display())))?;
    let paths = patch_paths(&text)?;
    for rel in &paths {
        dest_in(dir, rel)?;
    }
    // git is required. A hand-written applier that accepted a different
    // subset of diffs could leave a tree the patch does not describe, and a
    // verdict on that tree would be meaningless. `--check` first means a
    // patch that cannot apply cleanly is refused before any file changes.
    git_apply(dir, patch, true)?;
    git_apply(dir, patch, false)
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

fn copy_tree(from: &Path, to: &Path) -> Result<(), VerifyError> {
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
    fn walk(from: &Path, to: &Path, skip: &[&str]) -> Result<(), VerifyError> {
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
                std::fs::create_dir_all(&dest)
                    .map_err(|e| VerifyError(format!("{}: {e}", dest.display())))?;
                walk(&entry.path(), &dest, skip)?;
            } else if ty.is_file() {
                std::fs::copy(entry.path(), &dest)
                    .map_err(|e| VerifyError(format!("{}: {e}", dest.display())))?;
            }
        }
        Ok(())
    }
    walk(from, to, &skip)
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
        let out = verify(&req(&dir, forge, Some(poc_dir), None)).unwrap();
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
        let out = verify(&req(&dir, forge, Some(poc_dir), None)).unwrap();
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
        let out = verify(&req(&dir, forge, Some(poc_dir), None)).unwrap();
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
        let err = verify(&req(&dir, forge, Some(poc_dir), None)).unwrap_err();
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
        let out = verify(&Request {
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
        let err = verify(&Request {
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
        let err = verify(&req(&dir, forge, Some(poc_dir), None)).unwrap_err();
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
        let err = verify(&request).unwrap_err();
        assert!(
            err.0
                .contains("will not run an untrusted forge project on the host"),
            "{}",
            err.0
        );
        assert!(!marker.exists(), "forge was started without an attestation");
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
