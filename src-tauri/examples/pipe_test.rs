// End-to-end test: yt-dlp downloads + converts to MP3, rodio decodes and plays.
// Run: cargo run --example pipe_test

use std::io;
use std::process::Command;
use std::time::Duration;

use rodio::{Decoder, OutputStream, Sink};

fn main() {
    let video_id = "dQw4w9WgXcQ";
    let url = format!("https://www.youtube.com/watch?v={video_id}");
    let cache_dir = std::env::temp_dir().join("sunder");
    std::fs::create_dir_all(&cache_dir).expect("mkdir failed");

    let out_template = cache_dir.join(format!("{video_id}.%(ext)s"));
    let expected_path = cache_dir.join(format!("{video_id}.mp3"));

    eprintln!("[test] downloading + converting to MP3...");
    let output = Command::new("yt-dlp")
        .args([
            &url,
            "--extract-audio",
            "--audio-format", "mp3",
            "--audio-quality", "2",
            "-o", out_template.to_str().unwrap(),
            "--no-playlist",
            "--no-warnings",
            "--no-progress",
            "--force-overwrites",
        ])
        .output()
        .expect("failed to spawn yt-dlp");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("[test] FAIL: yt-dlp exited {}: {}", output.status, stderr.trim());
        return;
    }

    let meta = std::fs::metadata(&expected_path).expect("no output file");
    eprintln!("[test] MP3 ready: {} bytes", meta.len());

    eprintln!("[test] creating decoder...");
    let file = std::fs::File::open(&expected_path).expect("open failed");
    let decoder = Decoder::new(io::BufReader::new(file)).expect("decoder failed");
    eprintln!("[test] decoder created, starting playback...");

    let (_stream, handle) = OutputStream::try_default().expect("no audio device");
    let sink = Sink::try_new(&handle).expect("sink failed");
    sink.append(decoder);

    eprintln!("[test] playing for 5 seconds...");
    std::thread::sleep(Duration::from_secs(5));
    eprintln!("[test] position: {:?}", sink.get_pos());
    eprintln!("[test] SUCCESS - audio played");
    sink.stop();

    let _ = std::fs::remove_file(&expected_path);
}
