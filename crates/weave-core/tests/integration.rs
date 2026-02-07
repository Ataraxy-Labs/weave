use weave_core::entity_merge;

// =============================================================================
// Core value prop: independent entity changes auto-resolve
// =============================================================================

#[test]
fn ts_two_agents_add_different_functions() {
    let base = r#"import { config } from './config';

export function existing() {
    return config.value;
}
"#;
    let ours = r#"import { config } from './config';

export function existing() {
    return config.value;
}

export function validateToken(token: string): boolean {
    return token.length > 0 && token.startsWith("sk-");
}
"#;
    let theirs = r#"import { config } from './config';

export function existing() {
    return config.value;
}

export function formatDate(date: Date): string {
    return date.toISOString().split('T')[0];
}
"#;

    let result = entity_merge(base, ours, theirs, "utils.ts");
    assert!(
        result.is_clean(),
        "Two agents adding different functions should auto-resolve. Conflicts: {:?}",
        result.conflicts
    );
    assert!(result.content.contains("validateToken"));
    assert!(result.content.contains("formatDate"));
    assert!(result.content.contains("existing"));
}

#[test]
fn ts_one_modifies_one_adds() {
    let base = r#"export function greet(name: string) {
    return `Hello, ${name}`;
}
"#;
    let ours = r#"export function greet(name: string) {
    return `Hello, ${name}!`;
}
"#;
    let theirs = r#"export function greet(name: string) {
    return `Hello, ${name}`;
}

export function farewell(name: string) {
    return `Goodbye, ${name}`;
}
"#;

    let result = entity_merge(base, ours, theirs, "greetings.ts");
    assert!(
        result.is_clean(),
        "One modifying, one adding should auto-resolve. Conflicts: {:?}",
        result.conflicts
    );
    assert!(result.content.contains("Hello, ${name}!"));
    assert!(result.content.contains("farewell"));
}

// =============================================================================
// Real conflicts: same entity modified by both
// =============================================================================

#[test]
fn ts_both_modify_same_function_incompatibly() {
    let base = r#"export function process(data: any) {
    return data.toString();
}
"#;
    let ours = r#"export function process(data: any) {
    return JSON.stringify(data);
}
"#;
    let theirs = r#"export function process(data: any) {
    return data.toUpperCase();
}
"#;

    let result = entity_merge(base, ours, theirs, "process.ts");
    assert!(!result.is_clean());
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].entity_name, "process");
    // Should have enhanced conflict markers
    assert!(result.content.contains("<<<<<<< ours"));
    assert!(result.content.contains(">>>>>>> theirs"));
}

// =============================================================================
// Deletion scenarios
// =============================================================================

#[test]
fn ts_one_deletes_other_unchanged() {
    let base = r#"export function keep() {
    return 1;
}

export function remove() {
    return 2;
}
"#;
    let ours = r#"export function keep() {
    return 1;
}

export function remove() {
    return 2;
}
"#;
    // Theirs deletes `remove`
    let theirs = r#"export function keep() {
    return 1;
}
"#;

    let result = entity_merge(base, ours, theirs, "funcs.ts");
    assert!(
        result.is_clean(),
        "Delete of unchanged entity should auto-resolve. Conflicts: {:?}",
        result.conflicts
    );
    assert!(result.content.contains("keep"));
    assert!(!result.content.contains("remove"));
}

#[test]
fn ts_modify_delete_conflict() {
    let base = r#"export function shared() {
    return "original";
}
"#;
    // Ours modifies it
    let ours = r#"export function shared() {
    return "modified";
}
"#;
    // Theirs deletes it
    let theirs = "";

    let result = entity_merge(base, ours, theirs, "conflict.ts");
    assert!(
        !result.is_clean(),
        "Modify + delete should be a conflict"
    );
    assert_eq!(result.conflicts.len(), 1);
    assert!(
        result.content.contains("<<<<<<< ours"),
        "Should have conflict markers"
    );
}

// =============================================================================
// Python files
// =============================================================================

#[test]
fn py_two_agents_add_different_functions() {
    let base = r#"def existing():
    return 1
"#;
    let ours = r#"def existing():
    return 1

def agent_a_func():
    return "from agent A"
"#;
    let theirs = r#"def existing():
    return 1

def agent_b_func():
    return "from agent B"
"#;

    let result = entity_merge(base, ours, theirs, "module.py");
    assert!(
        result.is_clean(),
        "Python: different functions should auto-resolve. Conflicts: {:?}",
        result.conflicts
    );
    assert!(result.content.contains("agent_a_func"));
    assert!(result.content.contains("agent_b_func"));
}

// =============================================================================
// JSON files
// =============================================================================

#[test]
fn json_different_keys_modified() {
    let base = r#"{
  "name": "my-app",
  "version": "1.0.0",
  "description": "original"
}
"#;
    let ours = r#"{
  "name": "my-app",
  "version": "1.1.0",
  "description": "original"
}
"#;
    let theirs = r#"{
  "name": "my-app",
  "version": "1.0.0",
  "description": "updated description"
}
"#;

    let result = entity_merge(base, ours, theirs, "package.json");
    assert!(
        result.is_clean(),
        "JSON: different keys should auto-resolve. Conflicts: {:?}",
        result.conflicts
    );
    assert!(result.content.contains("1.1.0"));
    assert!(result.content.contains("updated description"));
}

// =============================================================================
// Commutative import merging
// =============================================================================

#[test]
fn ts_both_add_different_imports_no_conflict() {
    // Classic false conflict: both branches add different imports to the same block
    let base = r#"import { config } from './config';
import { logger } from './logger';

export function main() {
    logger.info(config.name);
}
"#;
    let ours = r#"import { config } from './config';
import { logger } from './logger';
import { validate } from './validate';

export function main() {
    logger.info(config.name);
}
"#;
    let theirs = r#"import { config } from './config';
import { logger } from './logger';
import { format } from './format';

export function main() {
    logger.info(config.name);
}
"#;

    let result = entity_merge(base, ours, theirs, "app.ts");
    assert!(
        result.is_clean(),
        "Both adding different imports should auto-resolve. Conflicts: {:?}",
        result.conflicts
    );
    assert!(result.content.contains("validate"), "Should contain ours import");
    assert!(result.content.contains("format"), "Should contain theirs import");
    assert!(result.content.contains("config"), "Should keep base imports");
    assert!(result.content.contains("logger"), "Should keep base imports");
}

#[test]
fn rust_both_add_different_use_statements() {
    let base = r#"use std::io;
use std::fs;

fn main() {
    println!("hello");
}
"#;
    let ours = r#"use std::io;
use std::fs;
use std::path::Path;

fn main() {
    println!("hello");
}
"#;
    let theirs = r#"use std::io;
use std::fs;
use std::collections::HashMap;

fn main() {
    println!("hello");
}
"#;

    let result = entity_merge(base, ours, theirs, "main.rs");
    assert!(
        result.is_clean(),
        "Rust: both adding different use statements should auto-resolve. Conflicts: {:?}",
        result.conflicts
    );
    assert!(result.content.contains("Path"), "Should contain ours use");
    assert!(result.content.contains("HashMap"), "Should contain theirs use");
}

#[test]
fn py_both_add_different_imports() {
    let base = r#"import os
import sys

def main():
    pass
"#;
    let ours = r#"import os
import sys
import json

def main():
    pass
"#;
    let theirs = r#"import os
import sys
import pathlib

def main():
    pass
"#;

    let result = entity_merge(base, ours, theirs, "app.py");
    assert!(
        result.is_clean(),
        "Python: both adding different imports should auto-resolve. Conflicts: {:?}",
        result.conflicts
    );
    assert!(result.content.contains("json"), "Should contain ours import");
    assert!(result.content.contains("pathlib"), "Should contain theirs import");
}

// =============================================================================
// Edge cases
// =============================================================================

#[test]
fn empty_base_both_add_same_content() {
    let base = "";
    let ours = r#"export function hello() {
    return "hello";
}
"#;
    let theirs = r#"export function hello() {
    return "hello";
}
"#;

    let result = entity_merge(base, ours, theirs, "new.ts");
    assert!(
        result.is_clean(),
        "Both adding identical content should resolve cleanly"
    );
}

#[test]
fn empty_base_both_add_different_content() {
    let base = "";
    let ours = r#"export function hello() {
    return "ours version";
}
"#;
    let theirs = r#"export function hello() {
    return "theirs version";
}
"#;

    let result = entity_merge(base, ours, theirs, "new.ts");
    assert!(
        !result.is_clean(),
        "Both adding different content for same function should conflict"
    );
}

#[test]
fn both_make_identical_changes() {
    let base = r#"export function shared() {
    return "old";
}
"#;
    let modified = r#"export function shared() {
    return "new";
}
"#;

    let result = entity_merge(base, modified, modified, "same.ts");
    assert!(result.is_clean());
    assert!(result.content.contains("new"));
}
