/// A clipboard that is only an HTML page URL. YouTube videos are handled apart.
pub fn page_url(text: &str) -> Option<&str> {
    let text = text.trim();
    if text.is_empty() || text.contains(char::is_whitespace) {
        return None;
    }
    if crate::youtube::video_id(text).is_some() {
        return None;
    }
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))?;
    if rest.is_empty() || rest.starts_with('/') {
        return None;
    }
    if is_file_url(rest) {
        return None;
    }
    Some(text)
}

pub fn host(url: &str) -> Option<&str> {
    let rest = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = host.split(':').next().unwrap_or(host);
    let host = host.strip_prefix("www.").unwrap_or(host);
    (!host.is_empty()).then_some(host)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlPreview {
    pub title: Option<String>,
    pub image: Option<String>,
}

pub fn from_html(html: &str, page: &str) -> HtmlPreview {
    let title = meta_content(html, "og:title")
        .or_else(|| meta_content(html, "twitter:title"))
        .or_else(|| title_tag(html));
    let image = meta_content(html, "og:image")
        .or_else(|| meta_content(html, "og:image:secure_url"))
        .or_else(|| meta_content(html, "twitter:image"))
        .or_else(|| meta_content(html, "twitter:image:src"))
        .and_then(|url| absolute_url(&url, page));
    HtmlPreview { title, image }
}

const FILE_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "avif", "svg", "pdf", "zip", "gz", "tgz", "mp4", "mp3",
    "mov", "webm", "wav", "css", "js", "mjs", "json", "xml", "txt", "csv", "tsv", "ico", "woff",
    "woff2", "ttf", "otf", "wasm", "heic", "bmp", "tif", "tiff",
];

fn is_file_url(rest: &str) -> bool {
    let path = match rest.find('/') {
        Some(index) => &rest[index..],
        None => return false,
    };
    let path = path.split(['?', '#']).next().unwrap_or(path);
    let file = path.rsplit('/').next().unwrap_or(path);
    let Some((_, ext)) = file.rsplit_once('.') else {
        return false;
    };
    FILE_EXTS.contains(&ext.to_ascii_lowercase().as_str())
}

fn meta_content(html: &str, key: &str) -> Option<String> {
    let mut rest = html;
    while let Some(start) = find_ci(rest, "<meta") {
        let after = &rest[start + 5..];
        let (tag, next) = split_tag(after);
        let matches = attr_is(tag, "property", key) || attr_is(tag, "name", key);
        if matches && let Some(content) = attr(tag, "content") {
            let decoded = decode_basic_entities(content.trim());
            if !decoded.is_empty() {
                return Some(decoded);
            }
        }
        if next.is_empty() {
            break;
        }
        rest = next;
    }
    None
}

fn title_tag(html: &str) -> Option<String> {
    let start = find_ci(html, "<title")?;
    let (_, inner) = split_tag(&html[start..]);
    let close = find_ci(inner, "</title")?;
    let text = decode_basic_entities(inner[..close].trim());
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!flat.is_empty()).then_some(flat)
}

fn attr_is(tag: &str, name: &str, key: &str) -> bool {
    attr(tag, name).is_some_and(|value| value.eq_ignore_ascii_case(key))
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let bytes = tag.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let name_start = i;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && bytes[i] != b'='
            && bytes[i] != b'>'
        {
            i += 1;
        }
        let attr_name = &tag[name_start..i];
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let quote = bytes[i];
        if quote != b'"' && quote != b'\'' {
            continue;
        }
        i += 1;
        let value_start = i;
        while i < bytes.len() && bytes[i] != quote {
            i += 1;
        }
        let value = &tag[value_start..i];
        if attr_name.eq_ignore_ascii_case(name) {
            return Some(value);
        }
        if i < bytes.len() {
            i += 1;
        }
    }
    None
}

fn split_tag(text: &str) -> (&str, &str) {
    let bytes = text.as_bytes();
    let mut quote = None;
    for (index, byte) in bytes.iter().enumerate() {
        match quote {
            Some(open) if *byte == open => quote = None,
            Some(_) => {}
            None if *byte == b'"' || *byte == b'\'' => quote = Some(*byte),
            None if *byte == b'>' => return (&text[..index], &text[index + 1..]),
            None => {}
        }
    }
    (text, "")
}

fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

fn absolute_url(url: &str, page: &str) -> Option<String> {
    let url = url.trim();
    if url.starts_with("https://") || url.starts_with("http://") {
        return Some(url.to_string());
    }
    if let Some(rest) = url.strip_prefix("//") {
        let scheme = page.split_once("://")?.0;
        return Some(format!("{scheme}://{rest}"));
    }
    let origin = origin(page)?;
    if let Some(path) = url.strip_prefix('/') {
        return Some(format!("{origin}/{path}"));
    }
    let dir = page_dir(page);
    Some(format!("{dir}{url}"))
}

fn origin(page: &str) -> Option<&str> {
    let (scheme, rest) = page.split_once("://")?;
    let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    if host_end == 0 {
        return None;
    }
    Some(&page[..scheme.len() + 3 + host_end])
}

fn page_dir(page: &str) -> String {
    let Some(origin) = origin(page) else {
        return page.to_string();
    };
    let path = &page[origin.len()..];
    let path = path.split(['?', '#']).next().unwrap_or(path);
    match path.rfind('/') {
        Some(index) => format!("{}{}", origin, &path[..=index]),
        None => format!("{origin}/"),
    }
}

fn decode_basic_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find(';') else {
            out.push('&');
            rest = after;
            continue;
        };
        let entity = &after[..end];
        let decoded = if let Some(hex) = entity
            .strip_prefix("#x")
            .or_else(|| entity.strip_prefix("#X"))
        {
            u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
        } else if let Some(decimal) = entity.strip_prefix('#') {
            decimal.parse::<u32>().ok().and_then(char::from_u32)
        } else {
            match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" | "nbsp" => Some(if entity == "nbsp" { '\u{00a0}' } else { '\'' }),
                _ => None,
            }
        };
        if let Some(ch) = decoded {
            out.push(ch);
            rest = &after[end + 1..];
        } else {
            out.push('&');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::{from_html, host, page_url};

    #[test]
    fn accepts_html_pages_and_skips_files() {
        assert_eq!(
            page_url("https://example.com/news/story"),
            Some("https://example.com/news/story")
        );
        assert_eq!(
            page_url("  https://www.example.com/index.html?x=1  "),
            Some("https://www.example.com/index.html?x=1")
        );
        assert_eq!(page_url("https://example.com"), Some("https://example.com"));
        assert_eq!(page_url("https://example.com/photo.jpg"), None);
        assert_eq!(page_url("https://example.com/file.pdf"), None);
        assert_eq!(
            page_url("https://www.youtube.com/watch?v=bEN9Dyg48b0"),
            None
        );
        assert_eq!(page_url("see https://example.com"), None);
    }

    #[test]
    fn reads_open_graph_preview() {
        let html = r#"
            <html><head>
            <title>Fallback</title>
            <meta content="https://cdn.example.com/a.jpg" property="og:image">
            <meta property="og:title" content="Hello &amp; Co">
            </head></html>
        "#;
        let preview = from_html(html, "https://example.com/news/story");
        assert_eq!(preview.title.as_deref(), Some("Hello & Co"));
        assert_eq!(
            preview.image.as_deref(),
            Some("https://cdn.example.com/a.jpg")
        );
    }

    #[test]
    fn resolves_relative_images_and_title_tag() {
        let html = r#"<title>  A   page </title><meta name="twitter:image" content="/pic.png">"#;
        let preview = from_html(html, "https://www.example.com/news/story");
        assert_eq!(preview.title.as_deref(), Some("A page"));
        assert_eq!(
            preview.image.as_deref(),
            Some("https://www.example.com/pic.png")
        );
        assert_eq!(
            host("https://www.example.com/news/story"),
            Some("example.com")
        );
    }

    #[test]
    fn protocol_relative_image_keeps_the_page_scheme() {
        let html = r#"<meta property="og:image" content="//cdn.example.com/a.jpg">"#;
        let preview = from_html(html, "http://example.com/a");
        assert_eq!(
            preview.image.as_deref(),
            Some("http://cdn.example.com/a.jpg")
        );
    }
}
