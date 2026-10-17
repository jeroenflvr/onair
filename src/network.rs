use std::{io, net::Ipv4Addr, process::Command};

pub fn parse_mac(value: &str) -> io::Result<[u8; 6]> {
    let mut mac = [0; 6];
    let parts: Vec<_> = value.split(':').collect();
    if parts.len() != mac.len() {
        return Err(io::Error::other("expected a six-byte MAC address"));
    }
    for (byte, part) in mac.iter_mut().zip(parts) {
        if part.is_empty() || part.len() > 2 || !part.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(io::Error::other("invalid MAC address byte"));
        }
        *byte = u8::from_str_radix(part, 16).map_err(io::Error::other)?;
    }
    Ok(mac)
}

fn gateway_and_interface(output: &str) -> Option<(Ipv4Addr, &str)> {
    let mut gateway = None;
    let mut interface = None;
    for line in output.lines() {
        if let Some((key, value)) = line.trim().split_once(':') {
            match key {
                "gateway" => gateway = value.trim().parse().ok(),
                "interface" => interface = Some(value.trim()),
                _ => {}
            }
        }
    }
    Some((gateway?, interface?))
}

fn router_matches(output: &str, expected: &[u8; 6]) -> io::Result<bool> {
    let words: Vec<_> = output.split_whitespace().collect();
    let address = words
        .windows(2)
        .find(|pair| pair[0] == "at")
        .map(|pair| pair[1])
        .ok_or_else(|| io::Error::other("could not read router MAC from arp output"))?;
    if address == "(incomplete)" {
        return Ok(false);
    }
    Ok(parse_mac(address)? == *expected)
}

pub fn at_home(expected: &[u8; 6]) -> io::Result<bool> {
    let route = Command::new("/sbin/route")
        .args(["-n", "get", "default"])
        .output()?;
    if !route.status.success() {
        return Err(io::Error::other(format!(
            "default gateway lookup failed: {}",
            String::from_utf8_lossy(&route.stderr).trim()
        )));
    }
    let route_text = String::from_utf8(route.stdout).map_err(io::Error::other)?;
    let Some((gateway, interface)) = gateway_and_interface(&route_text) else {
        // Tunnels and non-IPv4 gateways cannot establish a home-router match.
        return Ok(false);
    };
    let arp = Command::new("/usr/sbin/arp")
        .args(["-n", "-i", interface, &gateway.to_string()])
        .output()?;
    let arp_text = String::from_utf8(arp.stdout).map_err(io::Error::other)?;
    if !arp.status.success() {
        if arp.status.code() == Some(1) && arp_text.contains("no entry") {
            return Ok(false);
        }
        return Err(io::Error::other(format!(
            "router MAC lookup failed: {}",
            String::from_utf8_lossy(&arp.stderr).trim()
        )));
    }
    router_matches(&arp_text, expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_addresses_accept_macos_unpadded_bytes_and_case() {
        assert_eq!(
            parse_mac("2:10:18:3D:A5:83").unwrap(),
            parse_mac("02:10:18:3d:a5:83").unwrap()
        );
    }

    #[test]
    fn rejects_invalid_mac_addresses() {
        for value in ["", "02:10:18", "02:10:18:3d:a5:833", "02:10:18:3d:a5:gg"] {
            assert!(parse_mac(value).is_err(), "{value}");
        }
    }

    #[test]
    fn reads_gateway_and_interface() {
        assert_eq!(
            gateway_and_interface("    gateway: 192.168.0.1\n  interface: en0\n"),
            Some((Ipv4Addr::new(192, 168, 0, 1), "en0"))
        );
        assert_eq!(
            gateway_and_interface("gateway: link#20\ninterface: utun0"),
            None
        );
        assert_eq!(gateway_and_interface("interface: en0"), None);
    }

    #[test]
    fn matches_only_the_configured_router() {
        let home = parse_mac("02:10:18:3d:a5:83").unwrap();
        assert!(router_matches("? (192.168.0.1) at 2:10:18:3d:a5:83 on en0", &home).unwrap());
        assert!(!router_matches("? (192.168.0.1) at aa:bb:cc:dd:ee:ff on en0", &home).unwrap());
        assert!(!router_matches("? (192.168.0.1) at (incomplete) on en0", &home).unwrap());
        assert!(router_matches("unexpected output", &home).is_err());
    }
}
