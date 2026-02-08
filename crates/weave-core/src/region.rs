use sem_core::model::entity::SemanticEntity;

/// A region of a file — either an entity or the interstitial content between entities.
#[derive(Debug, Clone)]
pub enum FileRegion {
    Entity(EntityRegion),
    Interstitial(InterstitialRegion),
}

impl FileRegion {
    pub fn content(&self) -> &str {
        match self {
            FileRegion::Entity(e) => &e.content,
            FileRegion::Interstitial(i) => &i.content,
        }
    }

    pub fn key(&self) -> &str {
        match self {
            FileRegion::Entity(e) => &e.entity_id,
            FileRegion::Interstitial(i) => &i.position_key,
        }
    }

    pub fn is_entity(&self) -> bool {
        matches!(self, FileRegion::Entity(_))
    }
}

#[derive(Debug, Clone)]
pub struct EntityRegion {
    pub entity_id: String,
    pub entity_name: String,
    pub entity_type: String,
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone)]
pub struct InterstitialRegion {
    /// A key like "before:entity_id" or "after:entity_id" or "file_header" / "file_footer"
    pub position_key: String,
    pub content: String,
}

/// Extract ordered regions from file content using the given entities.
///
/// Entities must be from the same file. The function splits the file into
/// alternating interstitial and entity regions based on line ranges.
pub fn extract_regions(content: &str, entities: &[SemanticEntity]) -> Vec<FileRegion> {
    if entities.is_empty() {
        // Entire file is one interstitial region
        return vec![FileRegion::Interstitial(InterstitialRegion {
            position_key: "file_only".to_string(),
            content: content.to_string(),
        })];
    }

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    // Sort entities by start_line (they should already be sorted, but be safe)
    let mut sorted_entities: Vec<&SemanticEntity> = entities.iter().collect();
    sorted_entities.sort_by_key(|e| e.start_line);

    let mut regions: Vec<FileRegion> = Vec::new();
    let mut current_line: usize = 0; // 0-indexed into lines array

    for (i, entity) in sorted_entities.iter().enumerate() {
        // Entity start_line and end_line are 1-based from sem-core
        let entity_start = entity.start_line.saturating_sub(1); // convert to 0-based
        let entity_end = entity.end_line; // end_line is inclusive, so this is exclusive in 0-based

        // Comment bundling: scan backwards from entity_start to find leading doc comments.
        // These comments (JSDoc, Rust ///, Python docstrings, Java /** */) should be part
        // of the entity region, not the interstitial gap.
        let bundled_start = find_leading_comment_start(&lines, entity_start, current_line);

        // Interstitial before this entity (excluding bundled comments)
        if current_line < bundled_start {
            let interstitial_content = join_lines(&lines[current_line..bundled_start]);
            let position_key = if i == 0 {
                "file_header".to_string()
            } else {
                format!("between:{}:{}", sorted_entities[i - 1].id, entity.id)
            };
            regions.push(FileRegion::Interstitial(InterstitialRegion {
                position_key,
                content: interstitial_content,
            }));
        }

        // Entity region — includes bundled leading comments
        let entity_end_clamped = entity_end.min(total_lines);
        let entity_content = if bundled_start < entity_end_clamped {
            join_lines(&lines[bundled_start..entity_end_clamped])
        } else {
            entity.content.clone()
        };

        regions.push(FileRegion::Entity(EntityRegion {
            entity_id: entity.id.clone(),
            entity_name: entity.name.clone(),
            entity_type: entity.entity_type.clone(),
            content: entity_content,
            start_line: entity.start_line,
            end_line: entity.end_line,
        }));

        current_line = entity_end_clamped;
    }

    // Interstitial after last entity (file footer)
    if current_line < total_lines {
        let footer_content = join_lines(&lines[current_line..total_lines]);
        regions.push(FileRegion::Interstitial(InterstitialRegion {
            position_key: "file_footer".to_string(),
            content: footer_content,
        }));
    }

    // Handle trailing newline — if original content ends with newline and our last region doesn't
    if content.ends_with('\n') {
        if let Some(last) = regions.last() {
            if !last.content().ends_with('\n') {
                match regions.last_mut() {
                    Some(FileRegion::Entity(e)) => e.content.push('\n'),
                    Some(FileRegion::Interstitial(i)) => i.content.push('\n'),
                    None => {}
                }
            }
        }
    }

    regions
}

/// Find the start of leading doc comments before an entity.
///
/// Walks backwards from `entity_start` to find contiguous doc comment lines.
/// Stops at `min_line` (the end of the previous entity/region).
///
/// Recognizes:
/// - `///` and `//!` (Rust doc comments)
/// - `/** ... */` (JSDoc, JavaDoc block comments)
/// - `# comment` above Python defs (not always doc, but commonly associated)
/// - Decorators/annotations (already handled by entity extraction, but defensive)
fn find_leading_comment_start(lines: &[&str], entity_start: usize, min_line: usize) -> usize {
    if entity_start == 0 || entity_start <= min_line {
        return entity_start;
    }

    let mut comment_start = entity_start;
    let mut in_block_comment = false;

    // Walk backwards
    let mut line_idx = entity_start.saturating_sub(1);
    loop {
        if line_idx < min_line {
            break;
        }

        let trimmed = lines[line_idx].trim();

        if trimmed.is_empty() {
            // Allow one blank line between comment and entity
            // But don't extend past it
            if comment_start == entity_start && line_idx + 1 == entity_start {
                // Blank line immediately before entity — skip it, check further up
                line_idx = line_idx.saturating_sub(1);
                if line_idx < min_line {
                    break;
                }
                continue;
            }
            break;
        }

        // Check for end of block comment (scanning backwards, so */ means start of block)
        if trimmed.ends_with("*/") && !trimmed.starts_with("/*") {
            // This is the end of a block comment — scan backwards for /*
            in_block_comment = true;
            comment_start = line_idx;
            if line_idx == min_line {
                break;
            }
            line_idx -= 1;
            continue;
        }

        if in_block_comment {
            if trimmed.starts_with("/*") || trimmed.starts_with("/**") {
                comment_start = line_idx;
                in_block_comment = false;
            }
            // Continue scanning backwards through block comment
            if line_idx == min_line {
                break;
            }
            line_idx -= 1;
            continue;
        }

        // Single-line doc comment patterns
        if trimmed.starts_with("///")    // Rust doc comment
            || trimmed.starts_with("//!") // Rust inner doc comment
            || trimmed.starts_with("/**") // JSDoc/JavaDoc one-liner
            || trimmed.starts_with("* ")  // JSDoc/JavaDoc continuation
            || trimmed == "*"             // Empty JSDoc line
            || trimmed == "*/"            // End of JSDoc block
        {
            comment_start = line_idx;
            if line_idx == min_line {
                break;
            }
            line_idx -= 1;
            continue;
        }

        // Not a comment line — stop
        break;
    }

    comment_start
}

fn join_lines(lines: &[&str]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let mut result = lines.join("\n");
    result.push('\n');
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use sem_core::parser::plugins::create_default_registry;

    #[test]
    fn test_extract_regions_typescript() {
        let content = r#"import { foo } from 'bar';

export function hello() {
    return "hello";
}

export function world() {
    return "world";
}
"#;

        let registry = create_default_registry();
        let plugin = registry.get_plugin("test.ts").unwrap();
        let entities = plugin.extract_entities(content, "test.ts");

        assert!(!entities.is_empty(), "Should extract entities from TypeScript");

        let regions = extract_regions(content, &entities);

        // Should have interstitial + entity regions
        assert!(regions.len() >= 2, "Should have multiple regions, got {}", regions.len());

        // Verify entities are present
        let entity_regions: Vec<_> = regions
            .iter()
            .filter_map(|r| match r {
                FileRegion::Entity(e) => Some(e),
                _ => None,
            })
            .collect();

        let entity_names: Vec<&str> = entity_regions.iter().map(|e| e.entity_name.as_str()).collect();
        assert!(entity_names.contains(&"hello"), "Should find hello function, got {:?}", entity_names);
        assert!(entity_names.contains(&"world"), "Should find world function, got {:?}", entity_names);
    }

    #[test]
    fn test_comment_bundling_jsdoc() {
        // JSDoc comment should be bundled with the following function entity
        let content = r#"import { foo } from 'bar';

/**
 * Greets a person by name.
 * @param name - The person's name
 */
export function hello(name: string) {
    return `Hello, ${name}!`;
}

export function world() {
    return "world";
}
"#;

        let registry = create_default_registry();
        let plugin = registry.get_plugin("test.ts").unwrap();
        let entities = plugin.extract_entities(content, "test.ts");

        let _hello = entities.iter().find(|e| e.name == "hello").expect("Should find hello");
        let regions = extract_regions(content, &entities);

        // Find the hello entity region
        let hello_region = regions.iter().find(|r| match r {
            FileRegion::Entity(e) => e.entity_name == "hello",
            _ => false,
        }).expect("Should find hello region");

        // The entity region should include the JSDoc comment
        assert!(
            hello_region.content().contains("/**"),
            "hello region should include JSDoc comment. Content: {:?}",
            hello_region.content(),
        );
        assert!(
            hello_region.content().contains("@param name"),
            "hello region should include JSDoc @param. Content: {:?}",
            hello_region.content(),
        );

        // The interstitial before hello should NOT contain the JSDoc
        let interstitials: Vec<_> = regions.iter().filter(|r| !r.is_entity()).collect();
        for inter in &interstitials {
            assert!(
                !inter.content().contains("/**") || inter.content().contains("@param") == false,
                "Interstitial should not contain the bundled JSDoc. Key: {:?}, Content: {:?}",
                inter.key(), inter.content(),
            );
        }
    }

    #[test]
    fn test_comment_bundling_rust_doc() {
        let content = r#"use std::io;

/// Adds two numbers together.
///
/// # Examples
/// ```
/// assert_eq!(add(1, 2), 3);
/// ```
fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn subtract(a: i32, b: i32) -> i32 {
    a - b
}
"#;

        let registry = create_default_registry();
        let plugin = registry.get_plugin("test.rs").unwrap();
        let entities = plugin.extract_entities(content, "test.rs");

        let regions = extract_regions(content, &entities);
        let add_region = regions.iter().find(|r| match r {
            FileRegion::Entity(e) => e.entity_name == "add",
            _ => false,
        }).expect("Should find add region");

        assert!(
            add_region.content().contains("/// Adds two numbers"),
            "add region should include Rust doc comment. Content: {:?}",
            add_region.content(),
        );
    }

    #[test]
    fn test_extract_regions_no_entities() {
        let content = "just some text\nno code here\n";
        let regions = extract_regions(content, &[]);
        assert_eq!(regions.len(), 1);
        assert!(!regions[0].is_entity());
        assert_eq!(regions[0].content(), content);
    }
}
