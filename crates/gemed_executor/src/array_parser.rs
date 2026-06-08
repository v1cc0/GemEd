use regex::Regex;
use thiserror::Error;

const MAX_REGEX_PATTERN_LENGTH: usize = 256;
const MAX_REGEX_INPUT_LENGTH: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitMode {
    Newline,
    Delimiter,
    Regex,
}

impl SplitMode {
    pub fn from_wire(value: Option<&str>) -> Self {
        match value {
            Some("delimiter") => Self::Delimiter,
            Some("regex") => Self::Regex,
            _ => Self::Newline,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseArrayOptions {
    pub split_mode: SplitMode,
    pub delimiter: Option<String>,
    pub regex_pattern: Option<String>,
    pub trim_items: bool,
    pub remove_empty: bool,
}

impl Default for ParseArrayOptions {
    fn default() -> Self {
        Self {
            split_mode: SplitMode::Newline,
            delimiter: None,
            regex_pattern: None,
            trim_items: true,
            remove_empty: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseArrayResult {
    pub items: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Error)]
pub enum ArrayParseError {
    #[error("Regex pattern too long (max {MAX_REGEX_PATTERN_LENGTH} characters)")]
    PatternTooLong,
    #[error("Regex pattern rejected: nested quantifiers can cause catastrophic backtracking")]
    UnsafePattern,
    #[error("Input too long for regex mode (max {MAX_REGEX_INPUT_LENGTH} characters)")]
    InputTooLong,
    #[error("Invalid split pattern: {0}")]
    InvalidRegex(String),
}

pub fn parse_text_to_array(
    input_text: Option<&str>,
    options: &ParseArrayOptions,
) -> ParseArrayResult {
    match parse_text_to_array_checked(input_text, options) {
        Ok(items) => ParseArrayResult { items, error: None },
        Err(err) => ParseArrayResult {
            items: Vec::new(),
            error: Some(err.to_string()),
        },
    }
}

fn parse_text_to_array_checked(
    input_text: Option<&str>,
    options: &ParseArrayOptions,
) -> Result<Vec<String>, ArrayParseError> {
    let source = input_text.unwrap_or_default();
    if source.is_empty() {
        return Ok(Vec::new());
    }

    let raw_items: Vec<String> = match options.split_mode {
        SplitMode::Newline => source
            .split('\n')
            .map(|item| item.strip_suffix('\r').unwrap_or(item).to_string())
            .collect(),
        SplitMode::Delimiter => match options.delimiter.as_deref() {
            Some("") | None => vec![source.to_string()],
            Some(delimiter) => source.split(delimiter).map(ToOwned::to_owned).collect(),
        },
        SplitMode::Regex => {
            let Some(pattern) = options
                .regex_pattern
                .as_deref()
                .filter(|value| !value.is_empty())
            else {
                return Ok(vec![source.to_string()]);
            };
            if pattern.len() > MAX_REGEX_PATTERN_LENGTH {
                return Err(ArrayParseError::PatternTooLong);
            }
            if is_unsafe_pattern(pattern) {
                return Err(ArrayParseError::UnsafePattern);
            }
            if source.len() > MAX_REGEX_INPUT_LENGTH {
                return Err(ArrayParseError::InputTooLong);
            }
            let regex = parse_regex_pattern(pattern)?;
            regex.split(source).map(ToOwned::to_owned).collect()
        }
    };

    let mut items = raw_items;
    if options.trim_items {
        items = items
            .into_iter()
            .map(|item| item.trim().to_string())
            .collect();
    }
    if options.remove_empty {
        items.retain(|item| !item.is_empty());
    }

    Ok(items)
}

fn parse_regex_pattern(pattern: &str) -> Result<Regex, ArrayParseError> {
    if let Some((body, flags)) = parse_slash_regex(pattern) {
        let mut prefix = String::new();
        for flag in flags.chars() {
            match flag {
                'i' | 'm' | 's' | 'U' | 'x' => prefix.push(flag),
                'g' | 'u' | 'y' => {}
                _ => {
                    return Err(ArrayParseError::InvalidRegex(format!(
                        "unsupported flag `{flag}`"
                    )));
                }
            }
        }
        let compiled = if prefix.is_empty() {
            Regex::new(body)
        } else {
            Regex::new(&format!("(?{prefix}){body}"))
        };
        compiled.map_err(|err| ArrayParseError::InvalidRegex(err.to_string()))
    } else {
        Regex::new(pattern).map_err(|err| ArrayParseError::InvalidRegex(err.to_string()))
    }
}

fn parse_slash_regex(pattern: &str) -> Option<(&str, &str)> {
    if !pattern.starts_with('/') {
        return None;
    }
    let last_slash = pattern.rfind('/')?;
    if last_slash == 0 {
        return None;
    }
    let body = &pattern[1..last_slash];
    let flags = &pattern[last_slash + 1..];
    if flags.chars().all(|ch| ch.is_ascii_alphabetic()) {
        Some((body, flags))
    } else {
        None
    }
}

fn is_unsafe_pattern(pattern: &str) -> bool {
    let body = parse_slash_regex(pattern).map_or(pattern, |(body, _)| body);
    let mut depth = 0usize;
    let mut quantifier_at_depth = vec![false; 64];
    let mut chars = body.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            chars.next();
            continue;
        }
        match ch {
            '(' => {
                depth += 1;
                if depth >= quantifier_at_depth.len() {
                    quantifier_at_depth.resize(depth + 1, false);
                }
                quantifier_at_depth[depth] = false;
            }
            ')' => {
                let had_quantifier = quantifier_at_depth.get(depth).copied().unwrap_or(false);
                depth = depth.saturating_sub(1);
                let next = chars.peek().copied();
                if matches!(next, Some('+') | Some('*') | Some('{')) {
                    if had_quantifier {
                        return true;
                    }
                    quantifier_at_depth[depth] = true;
                } else if had_quantifier {
                    quantifier_at_depth[depth] = true;
                }
            }
            '+' | '*' if depth > 0 => quantifier_at_depth[depth] = true,
            _ => {}
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_newlines_like_legacy() {
        let result = parse_text_to_array(Some(" a\n\n b\r\n c "), &ParseArrayOptions::default());
        assert_eq!(result.items, ["a", "b", "c"]);
        assert_eq!(result.error, None);
    }

    #[test]
    fn parses_delimiter_mode() {
        let options = ParseArrayOptions {
            split_mode: SplitMode::Delimiter,
            delimiter: Some(",".to_string()),
            trim_items: true,
            remove_empty: true,
            regex_pattern: None,
        };
        let result = parse_text_to_array(Some("one, two,, three"), &options);
        assert_eq!(result.items, ["one", "two", "three"]);
    }

    #[test]
    fn rejects_unsafe_regex() {
        let options = ParseArrayOptions {
            split_mode: SplitMode::Regex,
            regex_pattern: Some("/(a+)+/".to_string()),
            ..ParseArrayOptions::default()
        };
        let result = parse_text_to_array(Some("aaaa"), &options);
        assert!(result.items.is_empty());
        assert!(result.error.unwrap().contains("nested quantifiers"));
    }
}
