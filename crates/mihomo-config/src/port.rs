use std::net::TcpListener;

/// Check if a port is available on localhost
pub fn is_port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

/// Find an available port starting from the given port
pub fn find_available_port(start_port: u16) -> Option<u16> {
    (start_port..start_port + 100).find(|&port| is_port_available(port))
}

/// Parse the port from a socket-ish address.
///
/// Accepts bare `host:port`, `:port`, bracketed IPv6 `[::1]:port`, and full
/// URLs such as `http://127.0.0.1:9090/path` (the shape
/// [`ConfigManager::get_external_controller`] returns).
///
/// Parsing is purely textual and **never resolves DNS**. The previous
/// `to_socket_addrs()` implementation only appeared to work because a
/// wildcard/fake-IP resolver happened to answer for a bogus host such as
/// `http://127.0.0.1`; in an offline / `unshare -n` test namespace DNS fails
/// and the parse returned `None`, so the same input behaved differently by
/// environment. This is deterministic everywhere.
pub fn parse_port_from_addr(addr: &str) -> Option<u16> {
    let trimmed = addr.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Drop an optional scheme (`http://`, `socks5://`, ...) and any path/query.
    let after_scheme = trimmed
        .rsplit_once("://")
        .map_or(trimmed, |(_, rest)| rest);
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(after_scheme);
    if let Some(rest) = authority.strip_prefix('[') {
        // Bracketed IPv6: `[::1]:9090`.
        let (_, port) = rest.rsplit_once("]:")?;
        return port.trim().parse().ok();
    }
    let (_, port) = authority.rsplit_once(':')?;
    let port = port.trim();
    if port.is_empty() {
        return None;
    }
    port.parse().ok()
}

/// DUAL-14-13: split a `host:port` listener address as the profile writes it.
///
/// The host may be empty (`:53`) and an IPv6 host may be bracketed
/// (`[::]:53`); a value with no port is not a listener address at all.
pub fn split_listen_addr(addr: &str) -> Option<(String, u16)> {
    let value = addr.trim();
    let (host, port) = value.rsplit_once(':')?;
    let port = port.trim().parse::<u16>().ok()?;
    let host = host.trim().trim_start_matches('[').trim_end_matches(']');
    Some((host.to_owned(), port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_port_available() {
        // Port 0 should always be available (OS assigns a free port)
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        // Port should not be available while listener is active
        assert!(!is_port_available(port));

        drop(listener);

        // Port should be available after listener is dropped
        assert!(is_port_available(port));
    }

    #[test]
    fn test_find_available_port() {
        let port = find_available_port(9090);
        assert!(port.is_some());
        assert!(port.unwrap() >= 9090);
    }

    #[test]
    fn test_parse_port_from_addr() {
        assert_eq!(parse_port_from_addr("127.0.0.1:9090"), Some(9090));
        assert_eq!(parse_port_from_addr("localhost:8080"), Some(8080));
        assert_eq!(parse_port_from_addr("invalid"), None);
        // URLs (the shape `get_external_controller` returns) and bracketed
        // IPv6 must parse without any DNS lookup, so the result is identical
        // online and inside an offline / `unshare -n` namespace.
        assert_eq!(parse_port_from_addr("http://127.0.0.1:9090"), Some(9090));
        assert_eq!(
            parse_port_from_addr("https://127.0.0.1:9090/path?q=1"),
            Some(9090)
        );
        assert_eq!(parse_port_from_addr(":9090"), Some(9090));
        assert_eq!(parse_port_from_addr("[::1]:9090"), Some(9090));
        assert_eq!(parse_port_from_addr("http://[::1]:9090"), Some(9090));
        assert_eq!(parse_port_from_addr("http://127.0.0.1"), None);
        assert_eq!(parse_port_from_addr(""), None);
    }

    #[test]
    fn test_split_listen_addr() {
        assert_eq!(
            split_listen_addr("127.0.0.1:1053"),
            Some(("127.0.0.1".to_owned(), 1053))
        );
        assert_eq!(split_listen_addr(":53"), Some((String::new(), 53)));
        assert_eq!(split_listen_addr("[::]:53"), Some(("::".to_owned(), 53)));
        assert_eq!(
            split_listen_addr(" 0.0.0.0:5353 "),
            Some(("0.0.0.0".to_owned(), 5353))
        );
        assert_eq!(split_listen_addr("1053"), None);
        assert_eq!(split_listen_addr("127.0.0.1:"), None);
        assert_eq!(split_listen_addr("127.0.0.1:http"), None);
        assert_eq!(split_listen_addr(""), None);
    }
}
