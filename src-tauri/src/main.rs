#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
fn install_stderr_filter() {
    // cpal's ALSA backend prints "A backend-specific error has occurred: ALSA
    // function 'snd_pcm_poll_descriptors' failed with error 'Unknown errno
    // (-32)'" (EPIPE) or (-86) (ESTRPIPE) every poll cycle when the audio
    // pipe is broken (USB DAC unplugged, PulseAudio sink suspended). The
    // Sunder underrun detector at engine.rs:979-1041 already handles EPIPE
    // silently, and cpal's worker at cpal-0.15.3/src/host/alsa/mod.rs:835-840
    // recovers internally. These stderr lines are pure noise; suppress them.
    //
    // Other cpal errors (ENODEV → "No such device", EBADF → "Bad file
    // descriptor", EINVAL → "Invalid argument") pass through because the
    // match predicate requires BOTH the function-name substring AND the
    // specific EPIPE/ESTRPIPE errno. Sunder's own eprintln lines do not
    // contain the substring.
    //
    // Implementation: libc::dup2 the pipe write-end onto fd 2 (stderr),
    // spawn a reader thread that writes non-matching lines to the original
    // stderr fd. Atomic-append buffer is line-by-line until '\n'.
    use std::io::Read;
    use std::os::unix::io::FromRawFd;
    use std::thread;

    let orig_fd = unsafe { libc::dup(2) };
    if orig_fd < 0 {
        return;
    }
    let mut pipe_fds = [0 as libc::c_int; 2];
    if unsafe { libc::pipe(pipe_fds.as_mut_ptr()) } != 0 {
        unsafe {
            libc::close(orig_fd);
        }
        return;
    }
    let read_fd = pipe_fds[0];
    let write_fd = pipe_fds[1];
    if unsafe { libc::dup2(write_fd, 2) } < 0 {
        unsafe {
            libc::close(read_fd);
            libc::close(write_fd);
            libc::close(orig_fd);
        }
        return;
    }
    unsafe {
        libc::close(write_fd);
    }

    let _ = thread::Builder::new()
        .name("sunder-stderr-filter".into())
        .spawn(move || {
            let mut reader = unsafe { std::fs::File::from_raw_fd(read_fd) };
            // orig_fd is a duplicated fd pointing to the ORIGINAL stderr,
            // captured BEFORE dup2 stole fd 2. Writing to orig_fd writes
            // to the real stderr, not the pipe. The previous version
            // captured `as_raw_fd()` (the value 2) and then dup2 made
            // fd 2 the pipe, creating a feedback loop that filled the
            // pipe and eventually caused SIGABRT.
            let mut buffer: Vec<u8> = Vec::with_capacity(4096);
            let mut byte = [0u8; 1];
            loop {
                byte[0] = 0;
                match reader.read(&mut byte) {
                    Ok(0) => break,
                    Ok(_) => {
                        if byte[0] == b'\n' {
                            let suppress = std::str::from_utf8(&buffer)
                                .map(|s| {
                                    s.contains("snd_pcm_poll_descriptors")
                                        && (s.contains("Unknown errno (-32)")
                                            || s.contains("Unknown errno (-86)"))
                                })
                                .unwrap_or(false);
                            if !suppress {
                                unsafe {
                                    let ptr = buffer.as_ptr();
                                    let len = buffer.len();
                                    if libc::write(orig_fd, ptr as *const _, len) < 0 {
                                        // best-effort; nothing else we can do
                                    }
                                    let nl: [u8; 1] = [b'\n'];
                                    let _ = libc::write(orig_fd, nl.as_ptr() as *const _, 1);
                                }
                            }
                            buffer.clear();
                        } else {
                            buffer.push(byte[0]);
                        }
                    }
                    Err(_) => break,
                }
            }
            // Drain any remaining buffered bytes (no trailing newline).
            if !buffer.is_empty() {
                unsafe {
                    let ptr = buffer.as_ptr();
                    let len = buffer.len();
                    let _ = libc::write(orig_fd, ptr as *const _, len);
                }
            }
            unsafe {
                libc::close(orig_fd);
            }
        });
}

fn main() {
    #[cfg(target_os = "linux")]
    {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        install_stderr_filter();
    }

    sunder_lib::run()
}
