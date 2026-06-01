//! Simple JSON path parser for JSON MUTATION (`json_remove`/`json_set`/
//! `json_replace`). Mutation walks a concrete path; it is NOT a query, so only a
//! simple subset is accepted:
//!
//! - optional `$` root
//! - `.key` object access (also bare leading `key`)
//! - `["key"]` / `['key']` quoted object access
//! - `[index]` array access
//!
//! Filter / wildcard / recursive-descent syntax (`*`, `..`, `[?(...)]`, `[1,2]`,
//! `[1:3]`) is REJECTED — that style is read-only (applicability + json_expression
//! use `serde_json_path`). A rejected or empty path yields `Err(ParsePathError)`,
//! and the caller fails that component open (skips it).

const MAX_PATH_LEN: usize = 500;

/// One segment of a parsed simple path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seg {
    Key(String),
    Index(usize),
}

/// Rejection reason for an unparseable / unsupported mutation path.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ParsePathError {
    #[error("empty path")]
    Empty,
    #[error("path exceeds {MAX_PATH_LEN} chars")]
    TooLong,
    #[error("unsupported path syntax (filter/wildcard/recursive are query-only)")]
    Unsupported,
    #[error("malformed path")]
    Malformed,
}

/// Parse a simple mutation path into segments. Rejects query-only syntax.
pub fn parse(path: &str) -> Result<Vec<Seg>, ParsePathError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(ParsePathError::Empty);
    }
    if trimmed.len() > MAX_PATH_LEN {
        return Err(ParsePathError::TooLong);
    }
    // Reject query-only syntax outright (cheap pre-check).
    if trimmed.contains('*') || trimmed.contains("..") || trimmed.contains('?') {
        return Err(ParsePathError::Unsupported);
    }

    let bytes = trimmed.as_bytes();
    let mut i = 0;

    // Optional leading `$` root.
    if bytes.first() == Some(&b'$') {
        i += 1;
    }

    let mut segs: Vec<Seg> = Vec::new();

    while i < bytes.len() {
        match bytes[i] {
            b'.' => {
                i += 1;
                let start = i;
                while i < bytes.len() && bytes[i] != b'.' && bytes[i] != b'[' {
                    i += 1;
                }
                if i == start {
                    return Err(ParsePathError::Malformed);
                }
                let key = &trimmed[start..i];
                segs.push(Seg::Key(key.to_string()));
            }
            b'[' => {
                // Find the matching close bracket.
                let close = trimmed[i..]
                    .find(']')
                    .map(|rel| i + rel)
                    .ok_or(ParsePathError::Malformed)?;
                let inner = trimmed[i + 1..close].trim();
                if inner.is_empty() {
                    return Err(ParsePathError::Malformed);
                }
                // Quoted key: ["x"] or ['x'].
                if (inner.starts_with('"') && inner.ends_with('"') && inner.len() >= 2)
                    || (inner.starts_with('\'') && inner.ends_with('\'') && inner.len() >= 2)
                {
                    let key = &inner[1..inner.len() - 1];
                    segs.push(Seg::Key(key.to_string()));
                } else if let Ok(idx) = inner.parse::<usize>() {
                    segs.push(Seg::Index(idx));
                } else {
                    // Slices, comma-lists, negative indices, etc. are unsupported.
                    return Err(ParsePathError::Unsupported);
                }
                i = close + 1;
            }
            _ => {
                // A bare leading key (no `$`/`.`), e.g. `user.name`.
                if segs.is_empty() {
                    let start = i;
                    while i < bytes.len() && bytes[i] != b'.' && bytes[i] != b'[' {
                        i += 1;
                    }
                    let key = &trimmed[start..i];
                    segs.push(Seg::Key(key.to_string()));
                } else {
                    return Err(ParsePathError::Malformed);
                }
            }
        }
    }

    if segs.is_empty() {
        return Err(ParsePathError::Empty);
    }
    Ok(segs)
}
