use std::fs;
use weave_core::parse_weave_conflicts;

pub fn run(file_path: &str, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;

    let conflicts = parse_weave_conflicts(&content);

    if json {
        let json_conflicts: Vec<serde_json::Value> = conflicts
            .iter()
            .map(|c| {
                serde_json::json!({
                    "entity": c.entity_name,
                    "kind": c.entity_kind,
                    "complexity": format!("{}", c.complexity),
                    "confidence": c.confidence,
                    "hint": c.hint,
                })
            })
            .collect();

        let output = serde_json::json!({
            "file": file_path,
            "conflict_count": conflicts.len(),
            "conflicts": json_conflicts,
        });

        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        if conflicts.is_empty() {
            println!("No weave conflicts found in {}", file_path);
            return Ok(());
        }

        println!("{} conflict(s) in {}\n", conflicts.len(), file_path);

        for (i, c) in conflicts.iter().enumerate() {
            println!(
                "  {}. {} `{}` ({}, confidence: {})",
                i + 1,
                c.entity_kind,
                c.entity_name,
                c.complexity,
                c.confidence
            );
            println!("     Hint: {}\n", c.hint);
        }
    }

    Ok(())
}
