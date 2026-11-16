// Run a native closure in a fresh fork of the already initialized CLI state.
// The caller must be single-threaded: only async-signal-safe operations are
// guaranteed after fork in a process with other live threads.

/// Execute a native record from a copy-on-write snapshot and return its JSON.
/// Child crashes, panics, protocol failures and explicit errors are reported as
/// infrastructure errors rather than converted into Python exceptions.
#[cfg(unix)]
pub fn run_json(
    body: impl FnOnce() -> Result<serde_json::Value, String>,
) -> Result<serde_json::Value, String> {
    use std::io::{Read, Write};
    use std::os::fd::FromRawFd;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let mut fds = [-1; 2];
    // SAFETY: fds points to two writable integers; ownership is handled below.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } == -1 {
        return Err(format!(
            "could not create isolation pipe: {}",
            std::io::Error::last_os_error()
        ));
    }
    // New pipe descriptors have no descriptor flags. Avoid leaking either end
    // into an exec'd descendant, which could otherwise delay EOF indefinitely.
    for fd in fds {
        // SAFETY: fd is a valid pipe descriptor and F_SETFD takes an integer flag.
        if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
            let error = std::io::Error::last_os_error();
            // SAFETY: both descriptors remain owned by this function.
            unsafe {
                libc::close(fds[0]);
                libc::close(fds[1]);
            }
            return Err(format!("could not configure isolation pipe: {error}"));
        }
    }
    // SAFETY: callers ensure the process is single-threaded at this point.
    let pid = unsafe { libc::fork() };
    if pid == -1 {
        let error = std::io::Error::last_os_error();
        // SAFETY: both descriptors were created by pipe and remain owned here.
        unsafe {
            libc::close(fds[0]);
            libc::close(fds[1]);
        }
        return Err(format!("could not fork isolated record: {error}"));
    }
    if pid == 0 {
        // SAFETY: the child owns its copies of both pipe descriptors.
        unsafe {
            libc::close(fds[0]);
        }
        // SAFETY: the write descriptor is valid and has a single File owner.
        let mut writer = unsafe { std::fs::File::from_raw_fd(fds[1]) };
        let outcome = match catch_unwind(AssertUnwindSafe(body)) {
            Ok(Ok(value)) => serde_json::json!({"ok": value}),
            Ok(Err(error)) => serde_json::json!({"error": error}),
            Err(payload) => {
                let message = payload
                    .downcast_ref::<&str>()
                    .copied()
                    .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                    .unwrap_or("non-string panic");
                serde_json::json!({"error": format!("isolated record panicked: {message}")})
            }
        };
        let encoded = serde_json::to_vec(&outcome)
            .unwrap_or_else(|_| br#"{"error":"could not serialize isolated result"}"#.to_vec());
        let status = if writer.write_all(&encoded).is_ok() {
            0
        } else {
            111
        };
        drop(writer);
        // SAFETY: do not flush inherited buffers or run destructors in the child.
        unsafe {
            libc::_exit(status);
        }
    }

    // SAFETY: the parent owns its descriptor copies; File closes the read end.
    unsafe {
        libc::close(fds[1]);
    }
    let mut reader = unsafe { std::fs::File::from_raw_fd(fds[0]) };
    let mut payload = Vec::new();
    // Drain before waitpid so a response larger than pipe capacity cannot block.
    let read_result = reader.read_to_end(&mut payload);
    drop(reader);
    let mut status = 0;
    loop {
        // SAFETY: pid is the child just forked; status points to writable storage.
        let waited = unsafe { libc::waitpid(pid, &mut status, 0) };
        if waited == pid {
            break;
        }
        if waited == -1 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(format!("could not reap isolated record: {error}"));
        }
    }
    if libc::WIFSIGNALED(status) {
        return Err(format!(
            "isolated record terminated by signal {}",
            libc::WTERMSIG(status)
        ));
    }
    if !libc::WIFEXITED(status) || libc::WEXITSTATUS(status) != 0 {
        return Err(format!(
            "isolated record exited with status {}",
            if libc::WIFEXITED(status) {
                libc::WEXITSTATUS(status)
            } else {
                -1
            }
        ));
    }
    read_result.map_err(|error| format!("could not read isolated result: {error}"))?;
    let envelope: serde_json::Value = serde_json::from_slice(&payload)
        .map_err(|error| format!("invalid isolated result: {error}"))?;
    let object = envelope
        .as_object()
        .ok_or_else(|| "invalid isolated result envelope".to_owned())?;
    match (object.get("ok"), object.get("error")) {
        (Some(value), None) if object.len() == 1 => Ok(value.clone()),
        (None, Some(serde_json::Value::String(error))) if object.len() == 1 => Err(error.clone()),
        _ => Err("invalid isolated result envelope".to_owned()),
    }
}

#[cfg(not(unix))]
pub fn run_json(
    body: impl FnOnce() -> Result<serde_json::Value, String>,
) -> Result<serde_json::Value, String> {
    body()
}
