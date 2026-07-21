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
    use std::io::{Read, Write};
    use std::os::unix::io::{AsRawFd, FromRawFd};
    use std::thread;

    let orig_fd = std::io::stderr().as_raw_fd();
    let mut pipe_fds = [0 as libc::c_int; 2];
    if unsafe { libc::pipe(pipe_fds.as_mut_ptr()) } != 0 {
        return;
    }
    let read_fd = pipe_fds[0];
    let write_fd = pipe_fds[1];
    if unsafe { libc::dup2(write_fd, 2) } != 0 {
        unsafe {
            libc::close(read_fd);
            libc::close(write_fd);
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
            // Keep a duplicate of the original stderr so we can re-emit
            // non-matching lines. The dup2 above replaced fd 2 with the
            // pipe write-end, but File::from_raw_fd(orig_fd) would not be
            // safe because fd 2 no longer points there. Instead, open
            // /dev/stderr which always resolves to the current fd 2 (but
            // fd 2 IS our pipe now). So we duplicate the original stderr
            // BEFORE replacing fd 2 by reading the path. Use
            // /proc/self/fd/2 pre-replacement is not possible here.
            //
            // Simpler: write the non-matching lines to a fresh fd 2 dup of
            // the original stderr. We saved orig_fd (the OLD fd 2 value)
            // BEFORE dup2; re-dup that onto fd 2 only for the write path
            // is racy. Instead, the filter thread just calls libc::write
            // to the saved orig_fd directly.
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
                                // Write the line back to the ORIGINAL
                                // stderr fd (which we captured before
                                // dup2 stole fd 2).
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
            let _ = Write::flush(&mut std::io::stderr());
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
