use std::fmt;

/// The type of conflict between two branches' changes to an entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictKind {
    /// Both branches modified the same entity and the changes couldn't be merged.
    BothModified,
    /// One branch modified the entity while the other deleted it.
    ModifyDelete { modified_in_ours: bool },
    /// Both branches added an entity with the same ID but different content.
    BothAdded,
}

impl fmt::Display for ConflictKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConflictKind::BothModified => write!(f, "both modified"),
            ConflictKind::ModifyDelete {
                modified_in_ours: true,
            } => write!(f, "modified in ours, deleted in theirs"),
            ConflictKind::ModifyDelete {
                modified_in_ours: false,
            } => write!(f, "deleted in ours, modified in theirs"),
            ConflictKind::BothAdded => write!(f, "both added"),
        }
    }
}

/// Conflict complexity classification (ConGra taxonomy, arXiv:2409.14121).
///
/// Helps agents and tools choose appropriate resolution strategies:
/// - Text: trivial, usually auto-resolvable (comment changes)
/// - Syntax: signature/type changes, may need type-checking
/// - Functional: body logic changes, needs careful review
/// - Composite variants indicate multiple dimensions of change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictComplexity {
    /// Only text/comment/string changes
    Text,
    /// Signature, type, or structural changes (no body changes)
    Syntax,
    /// Function body / logic changes
    Functional,
    /// Both text and syntax changes
    TextSyntax,
    /// Both text and functional changes
    TextFunctional,
    /// Both syntax and functional changes
    SyntaxFunctional,
    /// All three dimensions changed
    TextSyntaxFunctional,
    /// Could not classify (e.g., unknown entity type)
    Unknown,
}

impl fmt::Display for ConflictComplexity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConflictComplexity::Text => write!(f, "T"),
            ConflictComplexity::Syntax => write!(f, "S"),
            ConflictComplexity::Functional => write!(f, "F"),
            ConflictComplexity::TextSyntax => write!(f, "T+S"),
            ConflictComplexity::TextFunctional => write!(f, "T+F"),
            ConflictComplexity::SyntaxFunctional => write!(f, "S+F"),
            ConflictComplexity::TextSyntaxFunctional => write!(f, "T+S+F"),
            ConflictComplexity::Unknown => write!(f, "?"),
        }
    }
}

/// Classify conflict complexity by analyzing what changed between versions.
pub fn classify_conflict(base: Option<&str>, ours: Option<&str>, theirs: Option<&str>) -> ConflictComplexity {
    let base = base.unwrap_or("");
    let ours = ours.unwrap_or("");
    let theirs = theirs.unwrap_or("");

    // Compare ours and theirs changes vs base
    let ours_diff = classify_change(base, ours);
    let theirs_diff = classify_change(base, theirs);

    // Merge the dimensions
    let has_text = ours_diff.text || theirs_diff.text;
    let has_syntax = ours_diff.syntax || theirs_diff.syntax;
    let has_functional = ours_diff.functional || theirs_diff.functional;

    match (has_text, has_syntax, has_functional) {
        (true, false, false) => ConflictComplexity::Text,
        (false, true, false) => ConflictComplexity::Syntax,
        (false, false, true) => ConflictComplexity::Functional,
        (true, true, false) => ConflictComplexity::TextSyntax,
        (true, false, true) => ConflictComplexity::TextFunctional,
        (false, true, true) => ConflictComplexity::SyntaxFunctional,
        (true, true, true) => ConflictComplexity::TextSyntaxFunctional,
        (false, false, false) => ConflictComplexity::Unknown,
    }
}

struct ChangeDimensions {
    text: bool,
    syntax: bool,
    functional: bool,
}

fn classify_change(base: &str, modified: &str) -> ChangeDimensions {
    if base == modified {
        return ChangeDimensions {
            text: false,
            syntax: false,
            functional: false,
        };
    }

    let base_lines: Vec<&str> = base.lines().collect();
    let modified_lines: Vec<&str> = modified.lines().collect();

    let mut has_comment_change = false;
    let mut has_signature_change = false;
    let mut has_body_change = false;

    // Check first line (usually signature) separately
    let base_sig = base_lines.first().copied().unwrap_or("");
    let mod_sig = modified_lines.first().copied().unwrap_or("");
    if base_sig != mod_sig {
        if is_comment_line(base_sig) || is_comment_line(mod_sig) {
            has_comment_change = true;
        } else {
            has_signature_change = true;
        }
    }

    // Check body lines
    let base_body: Vec<&str> = base_lines.iter().skip(1).copied().collect();
    let mod_body: Vec<&str> = modified_lines.iter().skip(1).copied().collect();

    if base_body != mod_body {
        // Check if changes are only in comments
        let base_no_comments: Vec<&str> = base_body
            .iter()
            .filter(|l| !is_comment_line(l))
            .copied()
            .collect();
        let mod_no_comments: Vec<&str> = mod_body
            .iter()
            .filter(|l| !is_comment_line(l))
            .copied()
            .collect();

        if base_no_comments == mod_no_comments {
            has_comment_change = true;
        } else {
            has_body_change = true;
        }
    }

    ChangeDimensions {
        text: has_comment_change,
        syntax: has_signature_change,
        functional: has_body_change,
    }
}

fn is_comment_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with("*")
        || trimmed.starts_with("#")
        || trimmed.starts_with("\"\"\"")
        || trimmed.starts_with("'''")
}

/// A conflict on a specific entity.
#[derive(Debug, Clone)]
pub struct EntityConflict {
    pub entity_name: String,
    pub entity_type: String,
    pub kind: ConflictKind,
    pub complexity: ConflictComplexity,
    pub ours_content: Option<String>,
    pub theirs_content: Option<String>,
    pub base_content: Option<String>,
}

impl EntityConflict {
    /// Render this conflict as enhanced conflict markers.
    pub fn to_conflict_markers(&self) -> String {
        let label = format!(
            "{} `{}` ({}, complexity: {})",
            self.entity_type, self.entity_name, self.kind, self.complexity
        );
        let ours = self.ours_content.as_deref().unwrap_or("");
        let theirs = self.theirs_content.as_deref().unwrap_or("");

        let mut out = String::new();
        out.push_str(&format!("<<<<<<< ours — {}\n", label));
        out.push_str(ours);
        if !ours.is_empty() && !ours.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("=======\n");
        out.push_str(theirs);
        if !theirs.is_empty() && !theirs.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&format!(">>>>>>> theirs — {}\n", label));
        out
    }
}

/// Statistics about a merge operation.
#[derive(Debug, Clone, Default)]
pub struct MergeStats {
    pub entities_unchanged: usize,
    pub entities_ours_only: usize,
    pub entities_theirs_only: usize,
    pub entities_both_changed_merged: usize,
    pub entities_conflicted: usize,
    pub entities_added_ours: usize,
    pub entities_added_theirs: usize,
    pub entities_deleted: usize,
    pub used_fallback: bool,
    /// Entities that were auto-merged but reference other modified entities.
    pub semantic_warnings: usize,
}

impl MergeStats {
    pub fn has_conflicts(&self) -> bool {
        self.entities_conflicted > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_functional_conflict() {
        let base = "function foo() {\n    return 1;\n}\n";
        let ours = "function foo() {\n    return 2;\n}\n";
        let theirs = "function foo() {\n    return 3;\n}\n";
        assert_eq!(
            classify_conflict(Some(base), Some(ours), Some(theirs)),
            ConflictComplexity::Functional
        );
    }

    #[test]
    fn test_classify_syntax_conflict() {
        // Signature changed, body unchanged
        let base = "function foo(a: number) {\n    return a;\n}\n";
        let ours = "function foo(a: string) {\n    return a;\n}\n";
        let theirs = "function foo(a: boolean) {\n    return a;\n}\n";
        assert_eq!(
            classify_conflict(Some(base), Some(ours), Some(theirs)),
            ConflictComplexity::Syntax
        );
    }

    #[test]
    fn test_classify_text_conflict() {
        // Only comment changes
        let base = "// old comment\n    return 1;\n";
        let ours = "// ours comment\n    return 1;\n";
        let theirs = "// theirs comment\n    return 1;\n";
        assert_eq!(
            classify_conflict(Some(base), Some(ours), Some(theirs)),
            ConflictComplexity::Text
        );
    }

    #[test]
    fn test_classify_syntax_functional_conflict() {
        // Signature + body changed
        let base = "function foo(a: number) {\n    return a;\n}\n";
        let ours = "function foo(a: string) {\n    return a + 1;\n}\n";
        let theirs = "function foo(a: boolean) {\n    return a + 2;\n}\n";
        assert_eq!(
            classify_conflict(Some(base), Some(ours), Some(theirs)),
            ConflictComplexity::SyntaxFunctional
        );
    }

    #[test]
    fn test_classify_unknown_when_identical() {
        let content = "function foo() {\n    return 1;\n}\n";
        assert_eq!(
            classify_conflict(Some(content), Some(content), Some(content)),
            ConflictComplexity::Unknown
        );
    }

    #[test]
    fn test_classify_modify_delete() {
        // Theirs deleted (None), ours modified body
        // vs empty: both signature and body differ → SyntaxFunctional
        let base = "function foo() {\n    return 1;\n}\n";
        let ours = "function foo() {\n    return 2;\n}\n";
        assert_eq!(
            classify_conflict(Some(base), Some(ours), None),
            ConflictComplexity::SyntaxFunctional
        );
    }

    #[test]
    fn test_classify_both_added() {
        // No base → comparing each side against empty
        // Both signature and body differ from empty → SyntaxFunctional
        let ours = "function foo() {\n    return 1;\n}\n";
        let theirs = "function foo() {\n    return 2;\n}\n";
        assert_eq!(
            classify_conflict(None, Some(ours), Some(theirs)),
            ConflictComplexity::SyntaxFunctional
        );
    }

    #[test]
    fn test_conflict_markers_include_complexity() {
        let conflict = EntityConflict {
            entity_name: "foo".to_string(),
            entity_type: "function".to_string(),
            kind: ConflictKind::BothModified,
            complexity: ConflictComplexity::Functional,
            ours_content: Some("return 1;".to_string()),
            theirs_content: Some("return 2;".to_string()),
            base_content: Some("return 0;".to_string()),
        };
        let markers = conflict.to_conflict_markers();
        assert!(markers.contains("complexity: F"), "Markers should contain complexity: {}", markers);
    }
}

impl fmt::Display for MergeStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unchanged: {}", self.entities_unchanged)?;
        if self.entities_ours_only > 0 {
            write!(f, ", ours-only: {}", self.entities_ours_only)?;
        }
        if self.entities_theirs_only > 0 {
            write!(f, ", theirs-only: {}", self.entities_theirs_only)?;
        }
        if self.entities_both_changed_merged > 0 {
            write!(f, ", auto-merged: {}", self.entities_both_changed_merged)?;
        }
        if self.entities_added_ours > 0 {
            write!(f, ", added-ours: {}", self.entities_added_ours)?;
        }
        if self.entities_added_theirs > 0 {
            write!(f, ", added-theirs: {}", self.entities_added_theirs)?;
        }
        if self.entities_deleted > 0 {
            write!(f, ", deleted: {}", self.entities_deleted)?;
        }
        if self.entities_conflicted > 0 {
            write!(f, ", CONFLICTS: {}", self.entities_conflicted)?;
        }
        if self.semantic_warnings > 0 {
            write!(f, ", semantic-warnings: {}", self.semantic_warnings)?;
        }
        if self.used_fallback {
            write!(f, " (line-level fallback)")?;
        }
        Ok(())
    }
}
