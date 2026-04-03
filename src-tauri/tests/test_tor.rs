use termex_lib::ssh::proxy::ProxyType;

#[test]
fn test_proxy_type_tor_from_str() {
    assert_eq!(ProxyType::from_str("tor"), Some(ProxyType::Tor));
}

#[test]
fn test_proxy_type_tor_as_str() {
    assert_eq!(ProxyType::Tor.as_str(), "tor");
}

#[test]
fn test_proxy_type_tor_roundtrip() {
    let s = ProxyType::Tor.as_str();
    let parsed = ProxyType::from_str(s);
    assert_eq!(parsed, Some(ProxyType::Tor));
}

#[test]
fn test_proxy_type_socks5_unchanged() {
    assert_eq!(ProxyType::from_str("socks5"), Some(ProxyType::Socks5));
    assert_eq!(ProxyType::Socks5.as_str(), "socks5");
}

#[test]
fn test_proxy_type_unknown_returns_none() {
    assert_eq!(ProxyType::from_str("unknown"), None);
    assert_eq!(ProxyType::from_str("TOR"), None); // case sensitive
}
