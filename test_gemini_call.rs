use std::process::{Command, Stdio};

fn main() {
    let mut cmd = Command::new("gemini");
    cmd.env("CI", "true")
        .env("TERM", "dumb")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("-y")
        .arg("-m")
        .arg("gemini-3-flash-preview")
        .arg("-e")
        .arg("")
        .arg("-o")
        .arg("json")
        .arg("Say hello in one word");

    println!("Running command: {:?}", cmd);

    let output = cmd.output().expect("Failed to execute");

    println!("Status: {}", output.status);
    println!("Exit code: {:?}", output.status.code());
    println!("\nStdout:\n{}", String::from_utf8_lossy(&output.stdout));
    println!("\nStderr:\n{}", String::from_utf8_lossy(&output.stderr));
}
