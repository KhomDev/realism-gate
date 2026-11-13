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
//! to run when `foundry.toml` sets `ffi = true`, and the CLI refuses to run
//! on the host unless the operator passes `--sandbox` (an attestation that
//! the process is confined) or `--allow-host` (explicit, and still refused
//! when ffi is on). See `docs/verify-fix.md`.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    /// Whether the operator has accepted running forge here.
    pub host: HostPolicy,
}

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
    if ffi_enabled(&req.root) {
        return Err(VerifyError(
            "foundry.toml sets ffi = true. verify-fix will not run a project that can execute \
             host commands. Set ffi = false. A container with ffi off is the supported way to \
             run an untrusted PoC; see docs/verify-fix.md."
                .into(),
        ));
    }
    let _ = req.host;

    let mut temps = TempDirs::default();
    let baseline = temps.push(copy_of(&req.root)?);
    let patched = if let Some(dir) = &req.patched_dir {
        if !dir.is_dir() {
            return Err(VerifyError(format!("{}: not a directory", dir.display())));
        }
        temps.push(copy_of(dir)?)
    } else {
        let patch = req.patch.as_ref().expect("patch or dir");
        if !patch.is_file() {
            return Err(VerifyError(format!("{}: no such patch", patch.display())));
        }
        let work = temps.push(copy_of(&req.root)?);
        apply_patch(&work, patch)?;
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
    let output = cmd.output().map_err(|e| {
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

fn ffi_enabled(root: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(root.join("foundry.toml")) else {
        return false;
    };
    let stripped: String = text
        .lines()
        .map(|l| l.split('#').next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    stripped
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '='))
        .any(|w| {
            let w = w.trim();
            w.eq_ignore_ascii_case("ffi=true")
        })
        || stripped.to_ascii_lowercase().contains("ffi = true")
        || stripped.to_ascii_lowercase().contains("ffi=true")
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

fn apply_patch(dir: &Path, patch: &Path) -> Result<(), VerifyError> {
    let text = std::fs::read_to_string(patch)
        .map_err(|e| VerifyError(format!("{}: {e}", patch.display())))?;
    let paths = patch_paths(&text)?;
    for rel in &paths {
        dest_in(dir, rel)?;
    }
    let git = Command::new("git")
        .args(["apply", "--whitespace=nowarn"])
        .arg(patch)
        .current_dir(dir)
        .output();
    if let Ok(out) = git
        && out.status.success()
    {
        return Ok(());
    }
    apply_unified(dir, &text)
}

/// Paths a unified diff would write. Absolute paths and `..` are rejected
/// before any apply, so the patch cannot leave the temp tree.
fn patch_paths(text: &str) -> Result<Vec<String>, VerifyError> {
    let mut paths = Vec::new();
    for line in text.lines() {
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

fn apply_unified(dir: &Path, text: &str) -> Result<(), VerifyError> {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    let mut applied = 0;
    while i < lines.len() {
        if lines[i].starts_with("diff ")
            || lines[i].starts_with("index ")
            || lines[i].starts_with("rename ")
        {
            i += 1;
            continue;
        }
        if !lines[i].starts_with("--- ") {
            i += 1;
            continue;
        }
        i += 1;
        if i >= lines.len() || !lines[i].starts_with("+++ ") {
            return Err(VerifyError("malformed patch: missing +++".into()));
        }
        let new_path = strip_ab(lines[i].split_whitespace().nth(1).unwrap_or(""));
        i += 1;
        if new_path == "/dev/null" {
            return Err(VerifyError(
                "the patch deletes a file; verify-fix does not apply deletions".into(),
            ));
        }
        let dest = dest_in(dir, &new_path)?;
        let original = if dest.is_file() {
            std::fs::read_to_string(&dest)
                .map_err(|e| VerifyError(format!("{}: {e}", dest.display())))?
        } else {
            String::new()
        };
        let mut body: Vec<String> = original.lines().map(|s| s.to_string()).collect();
        let mut old_idx = 0usize;
        let mut rebuilt = Vec::new();
        if i >= lines.len() || !lines[i].starts_with("@@") {
            return Err(VerifyError("malformed patch: missing hunk".into()));
        }
        while i < lines.len() && lines[i].starts_with("@@") {
            let old_start = hunk_old_start(lines[i])?;
            i += 1;
            if old_start == 0 {
                return Err(VerifyError("malformed hunk".into()));
            }
            let keep_until = old_start - 1;
            if keep_until < old_idx || keep_until > body.len() {
                return Err(VerifyError(format!(
                    "hunk does not apply to {}",
                    dest.display()
                )));
            }
            rebuilt.extend(body[old_idx..keep_until].iter().cloned());
            old_idx = keep_until;
            while i < lines.len() && !lines[i].starts_with("@@") && !lines[i].starts_with("--- ") {
                let line = lines[i];
                i += 1;
                if line.starts_with('\\') {
                    continue;
                }
                let (tag, rest) =
                    line.split_at(line.chars().next().map(|c| c.len_utf8()).unwrap_or(0));
                match tag {
                    " " => {
                        if body.get(old_idx).map(String::as_str) != Some(rest)
                            && body.get(old_idx).map(String::as_str) != Some(line)
                        {
                            // A context line includes its leading space in `line`; `rest` is after it.
                        }
                        let ctx = line.strip_prefix(' ').unwrap_or(line);
                        if body.get(old_idx).map(String::as_str) != Some(ctx) {
                            return Err(VerifyError(format!(
                                "hunk context does not match {}",
                                dest.display()
                            )));
                        }
                        rebuilt.push(ctx.to_string());
                        old_idx += 1;
                    }
                    "-" => {
                        let ctx = line.strip_prefix('-').unwrap_or(line);
                        if body.get(old_idx).map(String::as_str) != Some(ctx) {
                            return Err(VerifyError(format!(
                                "hunk does not match {}",
                                dest.display()
                            )));
                        }
                        old_idx += 1;
                    }
                    "+" => {
                        rebuilt.push(line.strip_prefix('+').unwrap_or(line).to_string());
                    }
                    _ => {
                        return Err(VerifyError(format!("malformed hunk line `{line}`")));
                    }
                }
            }
        }
        rebuilt.extend(body.drain(old_idx..));
        body = rebuilt;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| VerifyError(format!("{}: {e}", parent.display())))?;
        }
        let mut text = body.join("\n");
        if original.ends_with('\n') || !text.is_empty() {
            text.push('\n');
        }
        std::fs::write(&dest, text).map_err(|e| VerifyError(format!("{}: {e}", dest.display())))?;
        applied += 1;
    }
    if applied == 0 {
        Err(VerifyError(
            "the patch did not apply. git apply failed and the diff had no hunk the built-in applier could use.".into(),
        ))
    } else {
        Ok(())
    }
}

fn hunk_old_start(header: &str) -> Result<usize, VerifyError> {
    // @@ -l,s +l,s @@
    let spec = header
        .split_whitespace()
        .find(|p| p.starts_with('-'))
        .ok_or_else(|| VerifyError(format!("malformed hunk `{header}`")))?;
    let n = spec.trim_start_matches('-').split(',').next().unwrap_or("");
    n.parse::<usize>()
        .map_err(|_| VerifyError(format!("malformed hunk `{header}`")))
}

fn copy_of(from: &Path) -> Result<PathBuf, VerifyError> {
    let to = std::env::temp_dir().join(format!(
        "realism-gate-verify-{}-{}",
        std::process::id(),
        unique()
    ));
    copy_tree(from, &to)?;
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

fn unique() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
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
            host: HostPolicy::Sandbox,
        }
    }

    #[test]
    fn a_poc_that_fails_on_both_sides_is_not_a_fix() {
        let dir = std::env::temp_dir().join(format!("rg-vf-{}", unique()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
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
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_passing_poc_means_the_fix_does_not_hold() {
        let dir = std::env::temp_dir().join(format!("rg-vp-{}", unique()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let forge = script(
            &dir,
            "forge",
            "#!/bin/sh\nprintf '%s\\n' '{\"suite\":{\"test_a\":{\"success\": true, \"logs\":[\"\\\"success\\\": false\"]}}}'\nexit 0\n",
        );
        let poc_dir = dir.join("patched");
        std::fs::create_dir_all(&poc_dir).unwrap();
        let out = verify(&req(&dir, forge, Some(poc_dir), None)).unwrap();
        assert!(matches!(out, Outcome::FixDoesNotHold { .. }), "{out:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_log_line_that_says_success_false_does_not_prove_a_fix() {
        let dir = std::env::temp_dir().join(format!("rg-vspoof-{}", unique()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
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
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_compile_error_is_not_a_failed_poc() {
        let dir = std::env::temp_dir().join(format!("rg-vc-{}", unique()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let forge = script(
            &dir,
            "forge",
            "#!/bin/sh\necho 'Compiler run failed' >&2\nexit 1\n",
        );
        let poc_dir = dir.join("patched");
        std::fs::create_dir_all(&poc_dir).unwrap();
        let err = verify(&req(&dir, forge, Some(poc_dir), None)).unwrap_err();
        assert!(err.0.contains("compile"), "{}", err.0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_patch_is_applied_before_forge_runs_and_the_baseline_must_pass() {
        let dir = std::env::temp_dir().join(format!("rg-vpatch-{}", unique()));
        let _ = std::fs::remove_dir_all(&dir);
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
            host: HostPolicy::Sandbox,
        })
        .unwrap();
        assert!(matches!(out, Outcome::FixHolds { .. }), "{out:?}");
        let original = std::fs::read_to_string(dir.join("src/A.sol")).unwrap();
        assert!(original.contains("x = 1"), "{original}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_patch_that_escapes_the_tree_is_refused() {
        let dir = std::env::temp_dir().join(format!("rg-vesc-{}", unique()));
        let _ = std::fs::remove_dir_all(&dir);
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
            host: HostPolicy::Sandbox,
        })
        .unwrap_err();
        assert!(
            err.0.contains("escapes") || err.0.contains(".."),
            "{}",
            err.0
        );
        assert!(!outside.exists(), "the patch wrote outside the temp tree");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ffi_in_foundry_toml_refuses_to_run() {
        let dir = std::env::temp_dir().join(format!("rg-vffi-{}", unique()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("foundry.toml"), "[profile.default]\nffi = true\n").unwrap();
        let forge = script(&dir, "forge", "#!/bin/sh\nexit 0\n");
        let poc_dir = dir.join("patched");
        std::fs::create_dir_all(&poc_dir).unwrap();
        let err = verify(&req(&dir, forge, Some(poc_dir), None)).unwrap_err();
        assert!(err.0.contains("ffi"), "{}", err.0);
        let _ = std::fs::remove_dir_all(&dir);
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
}
