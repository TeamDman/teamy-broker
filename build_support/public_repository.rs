/// Accept a public HTTPS repository identity without userinfo, query data or controls.
#[must_use]
pub fn public_repository(repository: Option<&str>) -> &str {
    let Some(repository) = repository else {
        return "unknown";
    };
    let Some(address) = repository.strip_prefix("https://") else {
        return "unknown";
    };
    let Some((host, path)) = address.split_once('/') else {
        return "unknown";
    };
    if host.is_empty()
        || path.is_empty()
        || !host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
        || !path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'/'))
    {
        return "unknown";
    }
    repository
}
