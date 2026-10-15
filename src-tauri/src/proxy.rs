// Windows' own proxy, as Settings (Network, Proxy) has it and as Clash or
// v2rayN set it when their "system proxy" is on, read for the crash reports
// (crash.rs: a URL for ureq). The registry's ProxyServer is "host:port" for
// every protocol, or one for each: "http=host:port;https=host:port;socks=host:port".
// A PAC script (AutoConfigURL, Clash's "PAC mode") is not read: as no proxy.

// Windows' proxy as set: ProxyServer.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct System {
    pub server: String,
}

impl System {
    // One protocol's entry of ProxyServer ("https=…"), when it has them.
    fn entry(&self, scheme: &str) -> Option<&str> {
        self.server.split(';').find_map(|part| {
            let (name, value) = part.split_once('=')?;
            Some(value.trim()).filter(|v| name.trim().eq_ignore_ascii_case(scheme) && !v.is_empty())
        })
    }

    // For the web (the crash reports' relays), as a URL: the https entry,
    // else the http one, else the one for every protocol.
    pub fn for_web(&self) -> Option<String> {
        let pick = if self.server.contains('=') { self.entry("https").or_else(|| self.entry("http"))? } else { self.server.trim() };
        if pick.is_empty() {
            return None;
        }
        Some(if pick.contains("://") { pick.to_string() } else { format!("http://{pick}") })
    }
}

// Windows' proxy, when one is on (and is not only a PAC script).
#[cfg(windows)]
pub fn system() -> Option<System> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings").ok()?;
    if key.get_value::<u32, _>("ProxyEnable").ok()? == 0 {
        return None;
    }
    let server: String = key.get_value("ProxyServer").ok()?;
    (!server.trim().is_empty()).then_some(System { server })
}

#[cfg(not(windows))]
pub fn system() -> Option<System> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sys(server: &str) -> System {
        System { server: server.into() }
    }

    #[test]
    fn what_clash_and_v2rayn_write_is_read() {
        // Clash (Verge, for Windows): its mixed port, for everything.
        assert_eq!(sys("127.0.0.1:7890").for_web().as_deref(), Some("http://127.0.0.1:7890"));
        // One for each protocol: the web through the https one.
        let each = sys("http=127.0.0.1:10809;https=127.0.0.1:10809;socks=127.0.0.1:10808");
        assert_eq!(each.for_web().as_deref(), Some("http://127.0.0.1:10809"));
    }
}
