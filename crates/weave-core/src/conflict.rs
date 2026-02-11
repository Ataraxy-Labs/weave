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

impl ConflictComplexity {
    /// Human-readable resolution hint for this conflict type.
    pub fn resolution_hint(&self) -> &'static str {
        match self {
            ConflictComplexity::Text =>
                "Cosmetic change on both sides. Pick either version or combine formatting.",
            ConflictComplexity::Syntax =>
                "Structural change (rename/retype). Check callers of this entity.",
            ConflictComplexity::Functional =>
                "Logic changed on both sides. Requires understanding intent of each change.",
            ConflictComplexity::TextSyntax =>
                "Renamed and reformatted. Prefer the structural change, verify formatting.",
            ConflictComplexity::TextFunctional =>
                "Logic and cosmetic changes overlap. Resolve logic first, then reformat.",
            ConflictComplexity::SyntaxFunctional =>
                "Structural and logic conflict. Both design and behavior differ.",
            ConflictComplexity::TextSyntaxFunctional =>
                "All three dimensions conflict. Manual review required.",
            ConflictComplexity::Unknown =>
                "Could not classify. Compare both versions manually.",
        }
    }
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
        let confidence = match &self.complexity {
            ConflictComplexity::Text => "high",
            ConflictComplexity::Syntax => "medium",
            ConflictComplexity::Functional => "medium",
            ConflictComplexity::TextSyntax => "medium",
            ConflictComplexity::TextFunctional => "medium",
            ConflictComplexity::SyntaxFunctional => "low",
            ConflictComplexity::TextSyntaxFunctional => "low",
            ConflictComplexity::Unknown => "unknown",
        };
        let label = format!(
            "{} `{}` ({}, confidence: {})",
            self.entity_type, self.entity_name, self.complexity, confidence
        );
        let hint = self.complexity.resolution_hint();
        let ours = self.ours_content.as_deref().unwrap_or("");
        let theirs = self.theirs_content.as_deref().unwrap_or("");

        let mut out = String::new();
        out.push_str(&format!("<<<<<<< ours \u{2014} {}\n", label));
        out.push_str(&format!("// hint: {}\n", hint));
        out.push_str(ours);
        if !ours.is_empty() && !ours.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("=======\n");
        out.push_str(theirs);
        if !theirs.is_empty() && !theirs.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&format!(">>>>>>> theirs \u{2014} {}\n", label));
        out
    }
}

/// A parsed conflict extracted from weave-enhanced conflict markers.
#[derive(Debug, Clone)]
pub struct ParsedConflict {
    pub entity_name: String,
    pub entity_kind: String,
    pub complexity: ConflictComplexity,
    pub confidence: String,
    pub hint: String,
    pub ours_content: String,
    pub theirs_content: String,
}

/// Parse weave-enhanced conflict markers from merged file content.
///
/// Returns a `Vec<ParsedConflict>` for each conflict block found.
/// Expects markers in the format produced by `EntityConflict::to_conflict_markers()`.
pub fn parse_weave_conflicts(content: &str) -> Vec<ParsedConflict> {
    let mut conflicts = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        // Look for <<<<<<< ours — <type> `<name>` (<complexity>, confidence: <conf>)
        if lines[i].starts_with("<<<<<<< ours") {
            let header = lines[i];
            let (entity_kind, entity_name, complexity, confidence) = parse_conflict_header(header);

            i += 1;

            // Read hint line
            let mut hint = String::new();
            if i < lines.len() && lines[i].starts_with("// hint: ") {
                hint = lines[i].trim_start_matches("// hint: ").to_string();
                i += 1;
            }

            // Read ours content until =======
            let mut ours_lines = Vec::new();
            while i < lines.len() && lines[i] != "=======" {
                ours_lines.push(lines[i]);
                i += 1;
            }
            i += 1; // skip =======

            // Read theirs content until >>>>>>>
            let mut theirs_lines = Vec::new();
            while i < lines.len() && !lines[i].starts_with(">>>>>>> theirs") {
                theirs_lines.push(lines[i]);
                i += 1;
            }
            i += 1; // skip >>>>>>>

            let ours_content = if ours_lines.is_empty() {
                String::new()
            } else {
                ours_lines.join("\n") + "\n"
            };
            let theirs_content = if theirs_lines.is_empty() {
                String::new()
            } else {
                theirs_lines.join("\n") + "\n"
            };

            conflicts.push(ParsedConflict {
                entity_name,
                entity_kind,
                complexity,
                confidence,
                hint,
                ours_content,
                theirs_content,
            });
        } else {
            i += 1;
        }
    }

    conflicts
}

fn parse_conflict_header(header: &str) -> (String, String, ConflictComplexity, String) {
    // Format: "<<<<<<< ours — <type> `<name>` (<complexity>, confidence: <conf>)"
    let after_dash = header
        .split('\u{2014}')
        .nth(1)
        .unwrap_or(header)
        .trim();

    // Extract entity type (word before backtick)
    let entity_kind = after_dash
        .split('`')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();

    // Extract entity name (between backticks)
    let entity_name = after_dash
        .split('`')
        .nth(1)
        .unwrap_or("")
        .to_string();

    // Extract complexity and confidence from parenthesized section
    let paren_content = after_dash
        .rsplit('(')
        .next()
        .unwrap_or("")
        .trim_end_matches(')');

    let parts: Vec<&str> = paren_content.split(',').map(|s| s.trim()).collect();
    let complexity = match parts.first().copied().unwrap_or("") {
        "T" => ConflictComplexity::Text,
        "S" => ConflictComplexity::Syntax,
        "F" => ConflictComplexity::Functional,
        "T+S" => ConflictComplexity::TextSyntax,
        "T+F" => ConflictComplexity::TextFunctional,
        "S+F" => ConflictComplexity::SyntaxFunctional,
        "T+S+F" => ConflictComplexity::TextSyntaxFunctional,
        _ => ConflictComplexity::Unknown,
    };

    let confidence = parts
        .iter()
        .find(|p| p.starts_with("confidence:"))
        .map(|p| p.trim_start_matches("confidence:").trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    (entity_kind, entity_name, complexity, confidence)
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
    /// Entities resolved via diffy 3-way merge (medium confidence).
    pub resolved_via_diffy: usize,
    /// Entities resolved via inner entity merge (high confidence).
    pub resolved_via_inner_merge: usize,
}

impl MergeStats {
    pub fn has_conflicts(&self) -> bool {
        self.entities_conflicted > 0
    }

    /// Overall merge confidence: High (only one side changed), Medium (diffy resolved),
    /// Low (inner entity merge or fallback), or Conflict.
    pub fn confidence(&self) -> &'static str {
        if self.entities_conflicted > 0 {
            "conflict"
        } else if self.resolved_via_inner_merge > 0 || self.used_fallback {
            "medium"
        } else if self.resolved_via_diffy > 0 {
            "high"
        } else {
            "very_high"
        }
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
    fn test_conflict_markers_include_complexity_and_hint() {
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
        assert!(markers.contains("confidence: medium"), "Markers should contain confidence: {}", markers);
        assert!(markers.contains("// hint: Logic changed on both sides"), "Markers should contain hint: {}", markers);
    }

    #[test]
    fn test_resolution_hints() {
        assert!(ConflictComplexity::Text.resolution_hint().contains("Cosmetic"));
        assert!(ConflictComplexity::Syntax.resolution_hint().contains("Structural"));
        assert!(ConflictComplexity::Functional.resolution_hint().contains("Logic"));
        assert!(ConflictComplexity::TextSyntax.resolution_hint().contains("Renamed"));
        assert!(ConflictComplexity::TextFunctional.resolution_hint().contains("Logic and cosmetic"));
        assert!(ConflictComplexity::SyntaxFunctional.resolution_hint().contains("Structural and logic"));
        assert!(ConflictComplexity::TextSyntaxFunctional.resolution_hint().contains("All three"));
        assert!(ConflictComplexity::Unknown.resolution_hint().contains("Could not classify"));
    }

    #[test]
    fn test_parse_weave_conflicts() {
        let conflict = EntityConflict {
            entity_name: "process".to_string(),
            entity_type: "function".to_string(),
            kind: ConflictKind::BothModified,
            complexity: ConflictComplexity::Functional,
            ours_content: Some("fn process() { return 1; }".to_string()),
            theirs_content: Some("fn process() { return 2; }".to_string()),
            base_content: Some("fn process() { return 0; }".to_string()),
        };
        let markers = conflict.to_conflict_markers();

        let parsed = parse_weave_conflicts(&markers);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].entity_name, "process");
        assert_eq!(parsed[0].entity_kind, "function");
        assert_eq!(parsed[0].complexity, ConflictComplexity::Functional);
        assert_eq!(parsed[0].confidence, "medium");
        assert!(parsed[0].hint.contains("Logic changed"));
        assert!(parsed[0].ours_content.contains("return 1"));
        assert!(parsed[0].theirs_content.contains("return 2"));
    }

    #[test]
    fn test_parse_weave_conflicts_multiple() {
        let c1 = EntityConflict {
            entity_name: "foo".to_string(),
            entity_type: "function".to_string(),
            kind: ConflictKind::BothModified,
            complexity: ConflictComplexity::Text,
            ours_content: Some("// a".to_string()),
            theirs_content: Some("// b".to_string()),
            base_content: None,
        };
        let c2 = EntityConflict {
            entity_name: "Bar".to_string(),
            entity_type: "class".to_string(),
            kind: ConflictKind::BothModified,
            complexity: ConflictComplexity::SyntaxFunctional,
            ours_content: Some("class Bar { x() {} }".to_string()),
            theirs_content: Some("class Bar { y() {} }".to_string()),
            base_content: None,
        };
        let content = format!("some code\n{}\nmore code\n{}\nend", c1.to_conflict_markers(), c2.to_conflict_markers());
        let parsed = parse_weave_conflicts(&content);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].entity_name, "foo");
        assert_eq!(parsed[0].complexity, ConflictComplexity::Text);
        assert_eq!(parsed[1].entity_name, "Bar");
        assert_eq!(parsed[1].complexity, ConflictComplexity::SyntaxFunctional);
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
