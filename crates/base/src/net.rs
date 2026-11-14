//! The address on the status row: the first IPv4 that is not loopback,
//! wired before wireless, because that is the one a person would type.

use std::net::IpAddr;

pub fn address() -> Option<String> {
    let mut best: Option<(u8, String)> = None;
    for iface in if_addrs::get_if_addrs().ok()? {
        if iface.is_loopback() {
            continue;
        }
        let IpAddr::V4(v4) = iface.ip() else {
            continue;
        };
        let rank = rank(&iface.name);
        if best.as_ref().is_none_or(|(r, _)| rank < *r) {
            best = Some((rank, v4.to_string()));
        }
    }
    best.map(|(_, a)| a)
}

fn rank(name: &str) -> u8 {
    if name.starts_with("eth") || name.starts_with("en") {
        0
    } else if name.starts_with("wl") {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wired_outranks_wireless_outranks_the_rest() {
        assert!(rank("eth0") < rank("wlan0"));
        assert!(rank("end0") < rank("wlan0"));
        assert!(rank("wlan0") < rank("docker0"));
    }

    #[test]
    fn address_answers_without_panicking() {
        let _ = address();
    }
}
