use std::{fmt, str::FromStr};

#[derive(Debug, Clone)]
pub enum UrlError {
    MissingScheme,
    InvalidScheme,
    MissingHost,
    UnexpectedFormat,
    DifferentSchemeOrHost,
}

impl std::error::Error for UrlError {}

impl fmt::Display for UrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UrlError::InvalidScheme => write!(f, "invalid url scheme"),
            UrlError::MissingScheme => write!(f, "missing url scheme"),
            UrlError::MissingHost => write!(f, "missing url host"),
            UrlError::UnexpectedFormat => write!(f, "unexpected url format"),
            UrlError::DifferentSchemeOrHost => {
                write!(f, "base url has different scheme or host from url or path")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UrlScheme {
    HTTP,
    HTTPS,
}

impl fmt::Display for UrlScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UrlScheme::HTTPS => write!(f, "https"),
            UrlScheme::HTTP => write!(f, "http"),
        }
    }
}

impl TryFrom<&str> for UrlScheme {
    type Error = UrlError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "http" => Ok(UrlScheme::HTTP),
            "https" => Ok(UrlScheme::HTTPS),
            _ => Err(UrlError::InvalidScheme),
        }
    }
}

impl fmt::Display for Url {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.path {
            Some(p) => write!(f, "{}://{}/{}", self.scheme, self.host, p),
            None => write!(f, "{}://{}", self.scheme, self.host),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Url {
    pub scheme: UrlScheme,
    pub host: String,
    pub path: Option<String>,
}

impl FromStr for Url {
    type Err = UrlError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (scheme, rest) = value.split_once("://").ok_or(UrlError::MissingScheme)?;
        let scheme = UrlScheme::try_from(scheme)?;

        let (host, path) = match rest.split_once("/") {
            Some((h, "")) => return Ok(Url::new(&scheme, h, None)),
            Some(parts) => parts,
            None => return Ok(Url::new(&scheme, rest, None)),
        };

        if host.is_empty() {
            return Err(UrlError::MissingHost);
        }

        let path = path
            .split_once('#')
            .map(|(without_fragments, _)| without_fragments)
            .unwrap_or(path)
            .trim_end_matches('/');

        if path.is_empty() {
            return Ok(Url::new(&scheme, host, None));
        }

        Ok(Url::new(&scheme, host, Some(path)))
    }
}

impl Url {
    pub fn new(scheme: &UrlScheme, host: &str, path: Option<&str>) -> Self {
        Url {
            scheme: scheme.to_owned(),
            host: host.to_owned(),
            path: path.map(|p| p.to_owned()),
        }
    }

    pub fn new_with_base(base_url: &Url, url_or_path: &str) -> Result<Self, UrlError> {
        if url_or_path.starts_with("http://") || url_or_path.starts_with("https://") {
            let url = Url::from_str(url_or_path);

            if let Ok(url) = url.as_ref() {
                if url.scheme != base_url.scheme || url.host != base_url.host {
                    return Err(UrlError::DifferentSchemeOrHost);
                }
            }

            return url;
        }

        if url_or_path.starts_with('/') {
            let path = if url_or_path == "/" {
                None
            } else {
                Some(url_or_path.trim_start_matches('/'))
            };

            return Ok(Url::new(&base_url.scheme, &base_url.host, path));
        }

        Err(UrlError::UnexpectedFormat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_urls_with_paths() {
        let url = Url::from_str("https://example.com/foo/bar/").unwrap();
        assert_eq!(url.scheme.to_string(), "https");
        assert_eq!(url.host, "example.com");
        assert_eq!(url.path, Some("foo/bar".to_string()));
        assert_eq!(url.to_string(), "https://example.com/foo/bar");
    }

    #[test]
    fn test_urls_with_fragments() {
        let url = Url::from_str("https://example.com/foo/bar#section1").unwrap();
        assert_eq!(url.path, Some("foo/bar".to_string()));
        assert_eq!(url.to_string(), "https://example.com/foo/bar");

        let url = Url::from_str("https://example.com/#top").unwrap();
        assert!(url.path.is_none());
        assert_eq!(url.to_string(), "https://example.com");
    }

    #[test]
    fn test_new_with_base_absolute_path() {
        let base = Url::from_str("https://example.com/").unwrap();

        let url = Url::new_with_base(&base, "/foo/bar").unwrap();
        assert_eq!(url.to_string(), "https://example.com/foo/bar");

        let url = Url::new_with_base(&base, "https://example.com/foo/bar").unwrap();
        assert_eq!(url.to_string(), "https://example.com/foo/bar");
    }

    #[test]
    fn test_display_format() {
        let url = Url::from_str("https://example.com/foo/bar").unwrap();
        assert_eq!(format!("{}", url), "https://example.com/foo/bar");
    }
}
