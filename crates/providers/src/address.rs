//! Which addresses a Provider may use. `https` is always allowed; plain `http` only for this
//! machine, the local network and Tailscale, so a key is never sent unencrypted across the internet.

use std::net::IpAddr;

use ureq::http::Uri;

/// Why an address is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressError {
    /// Not a web address with a scheme and a host.
    Invalid,
    /// Only `http` and `https` are spoken.
    UnsupportedScheme,
    /// `http` to a host that is not this machine, the local network or Tailscale.
    PlainHttpNotAllowed,
}

/// Checks an address typed for a Provider. Returns it trimmed, without a trailing slash.
pub fn check(raw: &str) -> Result<String, AddressError> {
    let trimmed = raw.trim().trim_end_matches('/');
    let uri: Uri = trimmed.parse().map_err(|_| AddressError::Invalid)?;
    let host = uri.host().ok_or(AddressError::Invalid)?;
    if uri.query().is_some() {
        return Err(AddressError::Invalid);
    }
    match uri.scheme_str() {
        Some("https") => Ok(trimmed.to_owned()),
        Some("http") if is_local(host) => Ok(trimmed.to_owned()),
        Some("http") => Err(AddressError::PlainHttpNotAllowed),
        Some(_) => Err(AddressError::UnsupportedScheme),
        None => Err(AddressError::Invalid),
    }
}

/// This machine, the local network (private, link-local and `.local` names) or Tailscale
/// (its CGNAT range, its IPv6 range and `.ts.net` names).
fn is_local(host: &str) -> bool {
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = bare.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(v4) => {
                let [a, b, ..] = v4.octets();
                v4.is_loopback()
                    || v4.is_private()
                    || v4.is_link_local()
                    // Tailscale's 100.64.0.0/10.
                    || (a == 100 && (b & 0xc0) == 64)
            }
            IpAddr::V6(v6) => {
                let first = v6.segments()[0];
                v6.is_loopback()
                    // Unique local (fc00::/7, which includes Tailscale's fd7a:115c:a1e0::/48).
                    || (first & 0xfe00) == 0xfc00
                    // Link-local (fe80::/10).
                    || (first & 0xffc0) == 0xfe80
            }
        };
    }
    let lower = bare.to_ascii_lowercase();
    let labels: Vec<&str> = lower.split('.').collect();
    lower == "localhost" || labels.last() == Some(&"local") || labels.ends_with(&["ts", "net"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_is_accepted_anywhere() {
        assert_eq!(
            check("https://api.example.com/v1/"),
            Ok("https://api.example.com/v1".into())
        );
    }

    #[test]
    fn plain_http_is_accepted_for_this_machine_the_lan_and_tailscale() {
        for ok in [
            "http://127.0.0.1:8080/v1",
            "http://localhost:8080/v1",
            "http://[::1]:8080/v1",
            "http://192.168.1.20:8080/v1",
            "http://10.0.0.5/v1",
            "http://172.20.1.1/v1",
            "http://169.254.10.10/v1",
            "http://100.101.4.7:8080/v1",
            "http://[fd7a:115c:a1e0::1]:8080/v1",
            "http://[fe80::1]:8080/v1",
            "http://mac-mini.local:8080/v1",
            "http://mac.tailnet.ts.net:8080/v1",
        ] {
            assert!(check(ok).is_ok(), "{ok} should be accepted");
        }
    }

    #[test]
    fn plain_http_to_the_internet_is_refused() {
        for bad in [
            "http://8.8.8.8/v1",
            "http://api.example.com/v1",
            "http://100.128.0.1/v1",
            "http://172.32.0.1/v1",
            "http://[2001:db8::1]/v1",
        ] {
            assert_eq!(check(bad), Err(AddressError::PlainHttpNotAllowed), "{bad}");
        }
    }

    #[test]
    fn only_web_schemes_with_a_host_are_valid() {
        assert_eq!(
            check("ftp://127.0.0.1/v1"),
            Err(AddressError::UnsupportedScheme)
        );
        assert_eq!(check("127.0.0.1:8080/v1"), Err(AddressError::Invalid));
        assert_eq!(check("https://"), Err(AddressError::Invalid));
        assert_eq!(check("  "), Err(AddressError::Invalid));
        assert_eq!(
            check("https://a.example/v1?x=1"),
            Err(AddressError::Invalid)
        );
    }
}
