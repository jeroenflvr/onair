use std::{io, process::Command};

fn command_output(program: &str, args: &[&str]) -> io::Result<String> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout).map_err(io::Error::other)
}

fn wifi_device(output: &str) -> Option<&str> {
    let mut lines = output.lines();
    while let Some(line) = lines.next() {
        if line.trim() == "Hardware Port: Wi-Fi" {
            return lines.next()?.trim().strip_prefix("Device:").map(str::trim);
        }
    }
    None
}

pub fn find_wifi_device() -> io::Result<String> {
    let output = command_output("/usr/sbin/networksetup", &["-listallhardwareports"])?;
    wifi_device(&output)
        .map(str::to_owned)
        .ok_or_else(|| io::Error::other("no Wi-Fi interface found"))
}

fn ssid(summary: &str) -> Option<&str> {
    summary.lines().find_map(|line| {
        let (key, value) = line.split_once(" : ")?;
        (key.trim() == "SSID").then_some(value.trim())
    })
}

fn current_ssid(device: &str) -> io::Result<Option<String>> {
    let summary = command_output("/usr/sbin/ipconfig", &["getsummary", device])?;
    match ssid(&summary) {
        Some("<redacted>") => Err(io::Error::other(
            "macOS redacted the Wi-Fi name; run `sudo ipconfig setverbose 1` once",
        )),
        Some(name) => Ok(Some(name.to_owned())),
        None => Ok(None),
    }
}

pub fn at_home(device: &str, home_ssid: &str) -> io::Result<bool> {
    Ok(current_ssid(device)?.as_deref() == Some(home_ssid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_wifi_device() {
        let output = "Hardware Port: Ethernet\nDevice: en1\n\nHardware Port: Wi-Fi\nDevice: en0\nEthernet Address: aa:bb\n";
        assert_eq!(wifi_device(output), Some("en0"));
        assert_eq!(wifi_device("Hardware Port: Ethernet\nDevice: en1\n"), None);
    }

    #[test]
    fn reads_ssid_from_summary() {
        let summary = "<dictionary> {\n  BSSID : aa:bb\n  SSID : My Home Net\n  Security : WPA2\n}";
        assert_eq!(ssid(summary), Some("My Home Net"));
        assert_eq!(ssid("<dictionary> {\n  BSSID : aa:bb\n}"), None);
    }
}
