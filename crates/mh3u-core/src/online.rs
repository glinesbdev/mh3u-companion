//! Telling whether the emulator is, or may go, online.
//!
//! The debug edits (`--debug-edit`) are for trying things out alone. The program does not want them used in online play, so it looks
//! for two signs: Cemu's own setting that turns online play on for the active account, and a network connection held by the emulator
//! process. This is a safeguard for honest users, not protection against someone who changes the program: it is open source.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Cemu's settings file: `$XDG_CONFIG_HOME/Cemu/settings.xml`, or `~/.config/Cemu/settings.xml`.
pub fn cemu_settings_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("Cemu/settings.xml"))
}

/// Whether the settings text turns online play on for the active account (`<OnlineEnabled>true</OnlineEnabled>`).
pub fn online_enabled_in(settings: &str) -> bool {
    settings
        .split("<OnlineEnabled>")
        .skip(1)
        .any(|rest| rest.trim_start().to_ascii_lowercase().starts_with("true"))
}

/// Whether Cemu's settings file, if it can be read, has online play on.
pub fn cemu_online_enabled(path: &Path) -> bool {
    std::fs::read_to_string(path).is_ok_and(|text| online_enabled_in(&text))
}

/// One row of `/proc/net/tcp`, `tcp6`, `udp` or `udp6`: the state, the remote address and the socket's inode.
struct Socket {
    state: u8,
    remote_is_loopback_or_none: bool,
    inode: u64,
}

fn sockets(text: &str) -> Vec<Socket> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            let remote = f.get(2)?;
            let (addr, port) = remote.rsplit_once(':')?;
            // an address in hex, in the host's byte order: 127.0.0.1 is 0100007F; ::1 ends in 01000000; all zeros is "no peer"
            let loopback = addr == "0100007F" || addr == "00000000000000000000000001000000";
            let none = addr.bytes().all(|b| b == b'0') && port == "0000";
            Some(Socket {
                state: u8::from_str_radix(f.get(3)?, 16).ok()?,
                remote_is_loopback_or_none: loopback || none,
                inode: f.get(9)?.parse().ok()?,
            })
        })
        .collect()
}

/// TCP connections that are up (state 1) to somewhere other than this machine, and UDP sockets with a peer elsewhere, among `owned`
/// socket inodes.
fn online_among(tcp: &[Socket], udp: &[Socket], owned: &HashSet<u64>) -> bool {
    tcp.iter()
        .any(|s| s.state == 1 && !s.remote_is_loopback_or_none && owned.contains(&s.inode))
        || udp.iter().any(|s| !s.remote_is_loopback_or_none && owned.contains(&s.inode))
}

/// Whether the process `pid` holds a network connection to another machine right now. Linux only; false where `/proc` cannot be read.
pub fn process_is_online(pid: u32) -> bool {
    let Ok(fds) = std::fs::read_dir(format!("/proc/{pid}/fd")) else {
        return false;
    };
    let owned: HashSet<u64> = fds
        .filter_map(|e| std::fs::read_link(e.ok()?.path()).ok())
        .filter_map(|target| {
            let text = target.to_string_lossy().into_owned();
            text.strip_prefix("socket:[")?.strip_suffix(']')?.parse().ok()
        })
        .collect();
    if owned.is_empty() {
        return false;
    }
    let read = |name: &str| std::fs::read_to_string(format!("/proc/{pid}/net/{name}")).unwrap_or_default();
    let mut tcp = sockets(&read("tcp"));
    tcp.extend(sockets(&read("tcp6")));
    let mut udp = sockets(&read("udp"));
    udp.extend(sockets(&read("udp6")));
    online_among(&tcp, &udp, &owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_online_setting_is_read_from_cemus_settings() {
        assert!(online_enabled_in("<Account>\n <OnlineEnabled>true</OnlineEnabled>\n</Account>"));
        assert!(online_enabled_in("<OnlineEnabled> True </OnlineEnabled>"));
        assert!(!online_enabled_in("<Account><OnlineEnabled>false</OnlineEnabled></Account>"));
        assert!(!online_enabled_in("<Account/>"));
        assert!(!online_enabled_in(""));
    }

    const HEADER: &str = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n";

    #[test]
    fn a_connection_to_another_machine_counts_and_a_local_one_does_not() {
        // an established connection to 34.1.2.3:443 (inode 100), one to 127.0.0.1 (101), a listener with no peer (102)
        let tcp = format!(
            "{HEADER}   0: 0100A8C0:C000 03020122:01BB 01 00000000:00000000 00:00000000 00000000  1000        0 100 1 0\n\
             \x20  1: 0100007F:C001 0100007F:1F90 01 00000000:00000000 00:00000000 00000000  1000        0 101 1 0\n\
             \x20  2: 00000000:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 102 1 0\n"
        );
        let tcp = sockets(&tcp);
        assert_eq!(tcp.len(), 3);
        let only = |n: u64| HashSet::from([n]);
        assert!(online_among(&tcp, &[], &only(100)));
        assert!(!online_among(&tcp, &[], &only(101)), "to this machine");
        assert!(!online_among(&tcp, &[], &only(102)), "listening only");
        assert!(!online_among(&tcp, &[], &only(999)), "not this process's socket");
    }

    #[test]
    fn a_udp_socket_with_a_peer_elsewhere_counts() {
        let udp = format!(
            "{HEADER}  10: 0100A8C0:D2F0 0A0B0C0D:1388 01 00000000:00000000 00:00000000 00000000  1000        0 200 2 0\n\
             \x20 11: 0100A8C0:D2F1 00000000:0000 07 00000000:00000000 00:00000000 00000000  1000        0 201 2 0\n"
        );
        let udp = sockets(&udp);
        assert!(online_among(&[], &udp, &HashSet::from([200])));
        assert!(!online_among(&[], &udp, &HashSet::from([201])), "bound, no peer");
    }

    #[test]
    fn this_process_is_not_online_and_a_missing_one_is_not_either() {
        assert!(!process_is_online(u32::MAX));
        // a test process holds no connection to another machine
        assert!(!process_is_online(std::process::id()));
    }
}
