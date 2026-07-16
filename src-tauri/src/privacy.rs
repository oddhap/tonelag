use url::Url;

pub fn redact_url(input: &str) -> String {
    let Ok(mut url) = Url::parse(input) else {
        return input.to_owned();
    };
    if !url.username().is_empty() {
        let _ = url.set_username("redacted");
    }
    if url.password().is_some() {
        let _ = url.set_password(Some("redacted"));
    }
    let pairs = url
        .query_pairs()
        .map(|(key, value)| {
            let sensitive = [
                "token", "password", "passwd", "secret", "api_key", "apikey", "auth",
            ]
            .iter()
            .any(|needle| key.to_ascii_lowercase().contains(needle));
            (
                key.into_owned(),
                if sensitive {
                    "redacted".into()
                } else {
                    value.into_owned()
                },
            )
        })
        .collect::<Vec<_>>();
    if !pairs.is_empty() {
        url.query_pairs_mut().clear().extend_pairs(pairs);
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_credentials_but_keeps_non_sensitive_query_values() {
        let result = redact_url("https://name:pass@example.test/live?token=abc&quality=high");
        assert!(!result.contains("name"));
        assert!(!result.contains("pass"));
        assert!(!result.contains("abc"));
        assert!(result.contains("quality=high"));
    }
}
