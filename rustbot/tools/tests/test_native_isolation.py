"""Exercise fork isolation in a standalone, single-threaded native process."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "crates/cli/src/isolation.rs"

FIXTURE = r'''
use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use serde_json::{json, Value};
include!(ISOLATION_SOURCE);

static SIGNAL_SEEN: AtomicBool = AtomicBool::new(false);
extern "C" fn note_signal(_: libc::c_int) { SIGNAL_SEEN.store(true, Ordering::SeqCst); }
fn assert_reaped() {
    let mut status = 0;
    let pid = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };
    assert_eq!(pid, -1, "a child was not reaped");
    assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(libc::ECHILD));
}
fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    let parent = Rc::new(Cell::new(7));
    let child = parent.clone();
    let value = run_json(move || {
        child.set(99);
        Ok(json!({"value": child.get(), "large": "é".repeat(120_000)}))
    }).unwrap();
    assert_eq!(parent.get(), 7);
    assert_eq!(value["value"], 99);
    assert_eq!(value["large"].as_str().unwrap().len(), 240_000);
    assert_reaped();
    assert_eq!(run_json(|| Ok(Value::Null)).unwrap(), Value::Null);
    assert_reaped();
    assert_eq!(run_json(|| Err("record failure".into())).unwrap_err(), "record failure");
    assert_reaped();
    let error = run_json(|| -> Result<Value, String> { panic!("failure in child") }).unwrap_err();
    assert!(error.contains("panicked") && error.contains("failure in child"), "{error}");
    assert_reaped();
    let error = run_json(|| -> Result<Value, String> {
        unsafe {
            let limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
            libc::setrlimit(libc::RLIMIT_CORE, &limit);
            libc::raise(libc::SIGABRT);
            libc::_exit(97);
        }
    }).unwrap_err();
    assert!(error.contains("signal") && error.contains(&libc::SIGABRT.to_string()), "{error}");
    assert_reaped();
    let error = run_json(|| -> Result<Value, String> {
        let child = unsafe { libc::fork() };
        if child < 0 { return Err("nested fork failed".into()); }
        if child == 0 {
            unsafe {
                let limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
                libc::setrlimit(libc::RLIMIT_CORE, &limit);
                libc::raise(libc::SIGABRT);
                libc::_exit(98);
            }
        }
        let mut status = 0;
        loop {
            let waited = unsafe { libc::waitpid(child, &mut status, 0) };
            if waited == child { break; }
            if waited == -1 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted { continue; }
            return Err("nested wait failed".into());
        }
        if libc::WIFSIGNALED(status) { Err(format!("nested signal {}", libc::WTERMSIG(status))) }
        else { Err("nested child unexpectedly survived".into()) }
    }).unwrap_err();
    assert!(error.contains(&format!("nested signal {}", libc::SIGABRT)), "{error}");
    assert_reaped();
    // Interrupt the parent's blocking pipe read while a single-threaded child
    // is working; read_to_end must retry EINTR and still collect the result.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = note_signal as libc::sighandler_t;
        libc::sigemptyset(&mut action.sa_mask);
        assert_eq!(libc::sigaction(libc::SIGUSR1, &action, std::ptr::null_mut()), 0);
    }
    let owner = unsafe { libc::getpid() };
    let interrupter = unsafe { libc::fork() };
    assert!(interrupter >= 0);
    if interrupter == 0 {
        unsafe { libc::usleep(150_000); libc::kill(owner, libc::SIGUSR1); libc::_exit(0); }
    }
    assert_eq!(run_json(|| { unsafe { libc::usleep(350_000); } Ok(json!("after signal")) }).unwrap(), json!("after signal"));
    let mut status=0;
    loop {
        let got=unsafe { libc::waitpid(interrupter, &mut status, 0) };
        if got==interrupter { break; }
        assert_eq!(std::io::Error::last_os_error().kind(), std::io::ErrorKind::Interrupted);
    }
    assert!(SIGNAL_SEEN.load(Ordering::SeqCst));
    assert_reaped();
    println!("isolation passed");
}
'''


@unittest.skipUnless(os.name == "posix", "fork isolation is Unix-only")
class NativeIsolationTest(unittest.TestCase):
    def test_fork_snapshot_pipe_errors_and_reaping(self):
        target = ROOT / "target"
        target.mkdir(exist_ok=True)
        cargo = shutil.which("cargo") or str(Path.home() / ".cargo/bin/cargo")
        with tempfile.TemporaryDirectory(prefix="isolation-test-", dir=target) as temp:
            project = Path(temp)
            (project / "src").mkdir()
            (project / "Cargo.toml").write_text(
                '[package]\nname="isolation_probe"\nversion="0.0.0"\nedition="2024"\n'
                '[dependencies]\nlibc="0.2"\nserde_json="1"\n[workspace]\n', encoding="utf8")
            source = FIXTURE.replace("ISOLATION_SOURCE", 'r#"' + str(SOURCE) + '"#')
            (project / "src/main.rs").write_text(source, encoding="utf8")
            build_target = target / "isolation-probe-build"
            env = dict(os.environ, CARGO_TARGET_DIR=str(build_target))
            build = subprocess.run(
                [cargo, "build", "--offline", "--manifest-path", str(project / "Cargo.toml")],
                env=env, text=True, capture_output=True, timeout=60,
            )
            self.assertEqual(build.returncode, 0, build.stderr)
            executable = build_target / "debug" / ("isolation_probe.exe" if sys.platform == "win32" else "isolation_probe")
            run = subprocess.run([str(executable)], text=True, capture_output=True, timeout=10)
            self.assertEqual(run.returncode, 0, run.stderr)
            self.assertEqual(run.stdout.strip(), "isolation passed")


if __name__ == "__main__":
    unittest.main()
