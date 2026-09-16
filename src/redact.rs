pub fn redact(text: &str) -> String {
    text.lines().map(redact_line).collect::<Vec<_>>().join("\n")
}

fn redact_line(line: &str) -> String {
    if let Some(redacted) = redact_labeled_line(line) {
        return redacted;
    }
    redact_inline(line)
}

fn redact_labeled_line(line: &str) -> Option<String> {
    let (label, value) = split_label(line)?;
    let key = normalize_label(label);
    if matches!(key.as_str(), "adres" | "address" | "straat") {
        return Some(format!("{label}: {}", address_region(value)));
    }
    if matches!(
        key.as_str(),
        "geboortedatum" | "geboorte datum" | "date of birth" | "dob"
    ) {
        return Some(format!("{label}: {}", age_band(value)));
    }
    if matches!(key.as_str(), "salaris" | "salary" | "inkomen") {
        return Some(format!("{label}: {}", salary_band(value)));
    }
    let replacement = match key.as_str() {
        "naam" | "name" | "voornaam" | "achternaam" | "full name" => Some("Verwijderd"),
        "e-mailadres" | "emailadres" | "e-mail" | "email" | "mail" => Some("Verwijderd"),
        "telefoonnummer" | "telefoon" | "phone" | "phonenumber" | "mobiel" => Some("Verwijderd"),
        "bsn" | "sofinummer" => Some("Verwijderd"),
        _ => None,
    };
    replacement.map(|r| format!("{label}: {r}"))
}

fn split_label(line: &str) -> Option<(&str, &str)> {
    let idx = line.find(':')?;
    let label = line[..idx].trim();
    let value = line[idx + 1..].trim();
    if label.is_empty() || value.is_empty() {
        return None;
    }
    if label.chars().count() > 40 {
        return None;
    }
    Some((label, value))
}

fn normalize_label(label: &str) -> String {
    label
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .filter(|c| *c != '.')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn address_region(value: &str) -> String {
    if let Some(city) = value
        .rsplit(',')
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let city = city
            .split_whitespace()
            .filter(|part| !is_postal_code_part(part))
            .collect::<Vec<_>>()
            .join(" ");
        if !city.is_empty() {
            return format!("Alleen regio ({city})");
        }
    }
    "Verwijderd".into()
}

fn is_postal_code_part(part: &str) -> bool {
    let chars: Vec<char> = part.chars().collect();
    matches!(chars.as_slice(), [a, b, c, d] if a.is_ascii_digit() && b.is_ascii_digit() && c.is_ascii_digit() && d.is_ascii_digit())
        || matches!(chars.as_slice(), [a, b] if a.is_ascii_alphabetic() && b.is_ascii_alphabetic())
}

fn age_band(value: &str) -> String {
    let year = value
        .split_whitespace()
        .filter_map(|part| part.parse::<i32>().ok())
        .find(|n| (1900..=2100).contains(n));
    let Some(year) = year else {
        return "Verwijderd".into();
    };
    let age = (2026 - year).clamp(0, 120);
    let lo = (age / 5) * 5;
    format!("Alleen leeftijdscategorie ({lo}-{} jaar)", lo + 5)
}

fn salary_band(value: &str) -> String {
    let digits: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return "Verwijderd".into();
    }
    let amount: u64 = digits.parse().unwrap_or(0);
    if amount == 0 {
        return "Verwijderd".into();
    }
    let step = if amount >= 1000 { 500 } else { 50 };
    let lo = (amount / step) * step;
    format!("€ {} - € {}", nl_amount(lo), nl_amount(lo + step))
}

fn nl_amount(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push('.');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

fn redact_inline(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if let Some((len, label)) = match_secret(&chars, i) {
            out.push_str(label);
            i += len;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn match_secret(chars: &[char], i: usize) -> Option<(usize, &'static str)> {
    match_url(chars, i)
        .or_else(|| match_email(chars, i))
        .or_else(|| match_token(chars, i))
        .or_else(|| match_iban(chars, i))
        .or_else(|| match_ipv6(chars, i))
        .or_else(|| match_ipv4(chars, i))
        .or_else(|| match_card(chars, i))
        .or_else(|| match_phone(chars, i))
}

fn match_url(chars: &[char], i: usize) -> Option<(usize, &'static str)> {
    let rest: String = chars[i..]
        .iter()
        .take(8)
        .collect::<String>()
        .to_ascii_lowercase();
    let prefix = if rest.starts_with("https://") {
        8
    } else if rest.starts_with("http://") {
        7
    } else {
        return None;
    };
    if i > 0 && chars[i - 1].is_ascii_alphanumeric() {
        return None;
    }
    let mut end = i + prefix;
    while end < chars.len() && !chars[end].is_whitespace() && chars[end] != '<' && chars[end] != '>'
    {
        end += 1;
    }
    if end == i + prefix {
        return None;
    }
    Some((end - i, "[url]"))
}

fn match_email(chars: &[char], i: usize) -> Option<(usize, &'static str)> {
    if !chars[i].is_ascii_alphanumeric() {
        return None;
    }
    if i > 0
        && (chars[i - 1].is_ascii_alphanumeric()
            || matches!(chars[i - 1], '.' | '_' | '%' | '+' | '-'))
    {
        return None;
    }
    let mut local_end = i;
    while local_end < chars.len()
        && (chars[local_end].is_ascii_alphanumeric()
            || matches!(chars[local_end], '.' | '_' | '%' | '+' | '-'))
    {
        local_end += 1;
    }
    if local_end == i || local_end >= chars.len() || chars[local_end] != '@' {
        return None;
    }
    let mut domain = local_end + 1;
    let domain_start = domain;
    let mut dots = 0;
    while domain < chars.len()
        && (chars[domain].is_ascii_alphanumeric() || chars[domain] == '.' || chars[domain] == '-')
    {
        if chars[domain] == '.' {
            dots += 1;
        }
        domain += 1;
    }
    if dots == 0 || domain - domain_start < 3 {
        return None;
    }
    Some((domain - i, "[email]"))
}

fn match_token(chars: &[char], i: usize) -> Option<(usize, &'static str)> {
    if i > 0 && chars[i - 1].is_ascii_alphanumeric() {
        return None;
    }
    const KEYS: &[&str] = &[
        "authorization",
        "password",
        "passwd",
        "api_key",
        "api-key",
        "apikey",
        "secret",
        "bearer",
        "token",
    ];
    let remaining: String = chars[i..]
        .iter()
        .take(24)
        .collect::<String>()
        .to_ascii_lowercase();
    let key = KEYS.iter().find(|k| remaining.starts_with(*k))?;
    let mut pos = i + key.len();
    while pos < chars.len() && chars[pos].is_whitespace() {
        pos += 1;
    }
    if pos >= chars.len() || (chars[pos] != '=' && chars[pos] != ':') {
        return None;
    }
    pos += 1;
    while pos < chars.len() && chars[pos].is_whitespace() {
        pos += 1;
    }
    if pos < chars.len() && (chars[pos] == '"' || chars[pos] == '\'') {
        pos += 1;
    }
    let value_start = pos;
    while pos < chars.len()
        && !chars[pos].is_whitespace()
        && !matches!(chars[pos], ',' | ';' | '"' | '\'')
    {
        pos += 1;
    }
    if pos == value_start {
        return None;
    }
    Some((pos - i, "[redacted]"))
}

fn match_iban(chars: &[char], i: usize) -> Option<(usize, &'static str)> {
    if i + 14 > chars.len() {
        return None;
    }
    if i > 0 && chars[i - 1].is_ascii_alphanumeric() {
        return None;
    }
    if !(chars[i].is_ascii_uppercase() && chars[i + 1].is_ascii_uppercase()) {
        return None;
    }
    if !(chars[i + 2].is_ascii_digit() && chars[i + 3].is_ascii_digit()) {
        return None;
    }
    let mut end = i + 4;
    while end < chars.len() && chars[end].is_ascii_alphanumeric() {
        end += 1;
    }
    let len = end - i;
    if !(14..=34).contains(&len) {
        return None;
    }
    if end < chars.len() && chars[end].is_ascii_alphanumeric() {
        return None;
    }
    Some((len, "[iban]"))
}

fn match_ipv4(chars: &[char], i: usize) -> Option<(usize, &'static str)> {
    if i > 0 && (chars[i - 1].is_ascii_digit() || chars[i - 1] == '.') {
        return None;
    }
    let mut pos = i;
    for part in 0..4 {
        if part > 0 {
            if pos >= chars.len() || chars[pos] != '.' {
                return None;
            }
            pos += 1;
        }
        let start = pos;
        while pos < chars.len() && chars[pos].is_ascii_digit() {
            pos += 1;
        }
        if pos == start || pos - start > 3 {
            return None;
        }
    }
    if pos < chars.len() && (chars[pos].is_ascii_digit() || chars[pos] == '.') {
        return None;
    }
    Some((pos - i, "[ip]"))
}

fn match_ipv6(chars: &[char], i: usize) -> Option<(usize, &'static str)> {
    if i > 0 && (chars[i - 1].is_ascii_hexdigit() || chars[i - 1] == ':') {
        return None;
    }
    let mut pos = i;
    let mut groups = 0;
    let mut colons = 0;
    while pos < chars.len() {
        if chars[pos] == ':' {
            colons += 1;
            pos += 1;
            continue;
        }
        if !chars[pos].is_ascii_hexdigit() {
            break;
        }
        let start = pos;
        while pos < chars.len() && chars[pos].is_ascii_hexdigit() && pos - start < 4 {
            pos += 1;
        }
        groups += 1;
        if pos < chars.len() && chars[pos] == ':' {
            continue;
        }
        break;
    }
    if groups < 3 || colons < 2 || pos - i < 5 {
        return None;
    }
    if pos < chars.len() && (chars[pos].is_ascii_hexdigit() || chars[pos] == ':') {
        return None;
    }
    Some((pos - i, "[ip]"))
}

fn match_card(chars: &[char], i: usize) -> Option<(usize, &'static str)> {
    if !chars[i].is_ascii_digit() {
        return None;
    }
    if i > 0 && (chars[i - 1].is_ascii_digit() || chars[i - 1] == '-' || chars[i - 1] == ' ') {
        return None;
    }
    let mut pos = i;
    let mut digits = 0;
    while pos < chars.len() {
        if chars[pos].is_ascii_digit() {
            digits += 1;
            pos += 1;
        } else if (chars[pos] == ' ' || chars[pos] == '-')
            && pos + 1 < chars.len()
            && chars[pos + 1].is_ascii_digit()
        {
            pos += 1;
        } else {
            break;
        }
    }
    if !(13..=19).contains(&digits) {
        return None;
    }
    Some((pos - i, "[card]"))
}

fn match_phone(chars: &[char], i: usize) -> Option<(usize, &'static str)> {
    let start = i;
    if chars[start] != '+' && !chars[start].is_ascii_digit() {
        return None;
    }
    if start > 0 && (chars[start - 1].is_ascii_digit() || chars[start - 1] == '+') {
        return None;
    }
    let mut pos = start;
    let mut digits = 0;
    if chars[pos] == '+' {
        pos += 1;
    }
    let mut last_digit = pos;
    while pos < chars.len() {
        if chars[pos].is_ascii_digit() {
            digits += 1;
            last_digit = pos + 1;
            pos += 1;
        } else if matches!(chars[pos], ' ' | '-' | '(' | ')') && digits > 0 {
            pos += 1;
        } else {
            break;
        }
    }
    if digits < 8 {
        return None;
    }
    Some((last_digit - i, "[phone]"))
}

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn redacts_email_and_url() {
        let out = redact("mail me at jane.doe@acme.com or https://secret.example/path?x=1");
        assert!(out.contains("[email]"));
        assert!(out.contains("[url]"));
        assert!(!out.contains("jane.doe"));
        assert!(!out.contains("secret.example"));
    }

    #[test]
    fn redacts_token_assignment() {
        let out = redact(r#"api_key = sk_live_abc123"#);
        assert!(out.contains("[redacted]"));
        assert!(!out.contains("sk_live_abc123"));
    }

    #[test]
    fn redacts_iban_and_ip() {
        let out = redact("pay NL91ABNA0417164300 from 192.168.1.10");
        assert!(out.contains("[iban]"));
        assert!(out.contains("[ip]"));
        assert!(!out.contains("NL91ABNA0417164300"));
        assert!(!out.contains("192.168.1.10"));
    }

    #[test]
    fn redacts_dutch_personal_record() {
        let src = "\
Naam: Jan de Vries
Adres: Hoofdstraat 45, 9711 AB Groningen
E-mailadres: jan.devries@email.nl
Telefoonnummer: 06-12345678
Geboortedatum: 12 mei 1984
Salaris: € 3.450";
        let out = redact(src);
        assert!(out.contains("Naam: Verwijderd"));
        assert!(out.contains("Adres: Alleen regio (Groningen)"));
        assert!(out.contains("E-mailadres: Verwijderd"));
        assert!(out.contains("Telefoonnummer: Verwijderd"));
        assert!(out.contains("Geboortedatum: Alleen leeftijdscategorie (40-45 jaar)"));
        assert!(out.contains("Salaris: € 3.000 - € 3.500"));
        assert!(!out.contains("Jan de Vries"));
        assert!(!out.contains("jan.devries"));
        assert!(!out.contains("06-12345678"));
    }
}
