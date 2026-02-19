use std::process::Command;

pub fn notify(title: &str, message: &str) {
    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        escape_applescript_string(message),
        escape_applescript_string(title)
    );

    let result = Command::new("osascript").arg("-e").arg(&script).output();

    if let Err(e) = result {
        tracing::error!("Failed to send notification: {}", e);
    }
}

fn escape_applescript_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
