#[derive(Debug, Clone, serde::Deserialize)]
pub struct Snippet {
    pub trigger: String,
    pub expand: String,
}

/// Expands snippet triggers in text
///
/// Matching rules:
/// - If entire text (trimmed) exactly matches a trigger → expand
/// - If text starts with trigger + space → expand snippet + append rest
/// - Otherwise → return None (no match)
pub fn expand_snippets(text: &str, snippets: &[Snippet]) -> Option<String> {
    let trimmed = text.trim();

    // Check exact match
    for snippet in snippets {
        if trimmed == snippet.trigger {
            return Some(snippet.expand.clone());
        }
    }

    // Check prefix match
    for snippet in snippets {
        if trimmed.starts_with(&format!("{} ", snippet.trigger)) {
            let rest = &trimmed[snippet.trigger.len()..];
            return Some(format!("{}{}", snippet.expand, rest));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_snippets() -> Vec<Snippet> {
        vec![
            Snippet {
                trigger: ":sig".to_string(),
                expand: "Best regards,\nZack".to_string(),
            },
            Snippet {
                trigger: ":react".to_string(),
                expand: "import React from 'react';".to_string(),
            },
        ]
    }

    #[test]
    fn test_exact_match() {
        let snippets = mock_snippets();
        assert_eq!(
            expand_snippets(":sig", &snippets),
            Some("Best regards,\nZack".to_string())
        );
    }

    #[test]
    fn test_prefix_match() {
        let snippets = mock_snippets();
        assert_eq!(
            expand_snippets(":sig hello", &snippets),
            Some("Best regards,\nZack hello".to_string())
        );
    }

    #[test]
    fn test_no_match() {
        let snippets = mock_snippets();
        assert_eq!(expand_snippets("hello world", &snippets), None);
        assert_eq!(expand_snippets(":signature", &snippets), None);
    }

    #[test]
    fn test_multiple_snippets() {
        let snippets = vec![
            Snippet {
                trigger: ":a".to_string(),
                expand: "Alpha".to_string(),
            },
            Snippet {
                trigger: ":ab".to_string(),
                expand: "Abbey".to_string(),
            },
        ];
        // First match wins based on iteration order
        assert_eq!(expand_snippets(":a", &snippets), Some("Alpha".to_string()));
        assert_eq!(expand_snippets(":ab", &snippets), Some("Abbey".to_string()));
        assert_eq!(
            expand_snippets(":a something", &snippets),
            Some("Alpha something".to_string())
        );
    }
}
