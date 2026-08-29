//! Child-process spawn policy: never flash a console window on Windows.
//!
//! Sunder is a GUI-subsystem app. On Windows, spawning a console-subsystem
//! child (yt-dlp, ffmpeg, ...) without `CREATE_NO_WINDOW` pops up a blank
//! terminal window that lives as long as the child. Every spawn site chains
//! `.no_window()` so the mistake is centralized here instead of duplicated
//! at nine call sites. No-op on every other platform.

/// `CREATE_NO_WINDOW` from the Win32 process creation flags.
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Chain on every spawned `Command` so children never open a console window.
pub trait NoWindow {
    fn no_window(&mut self) -> &mut Self;
}

impl NoWindow for tokio::process::Command {
    #[cfg(windows)]
    fn no_window(&mut self) -> &mut Self {
        // tokio::process::Command has an inherent `creation_flags` (Windows
        // only); it forwards to std's CommandExt internally.
        self.creation_flags(CREATE_NO_WINDOW);
        self
    }

    #[cfg(not(windows))]
    fn no_window(&mut self) -> &mut Self {
        self
    }
}

impl NoWindow for std::process::Command {
    #[cfg(windows)]
    fn no_window(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        self.creation_flags(CREATE_NO_WINDOW);
        self
    }

    #[cfg(not(windows))]
    fn no_window(&mut self) -> &mut Self {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::NoWindow;

    /// Off-Windows this is a no-op; on Windows it must not panic and must
    /// return the same command so builder-style chaining keeps working.
    #[test]
    fn no_window_returns_the_command_for_chaining() {
        let mut cmd = tokio::process::Command::new("yt-dlp");
        let chained = cmd.arg("--version").no_window().arg("-j");
        assert!(std::ptr::eq(chained, &mut cmd));

        let mut cmd = std::process::Command::new("yt-dlp");
        let chained = cmd.arg("--version").no_window().arg("-j");
        assert!(std::ptr::eq(chained, &mut cmd));
    }
}
