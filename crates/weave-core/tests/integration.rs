use weave_core::{conflict::ConflictKind, entity_merge};

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

#[test]
fn ts_same_named_function_added_at_different_positions_conflicts() {
    let base = r#"function first() { return 1; }
function second() { return 2; }
"#;
    let ours = r#"function first() { return 1; }
function computeKey(input) { return input.id; }
function second() { return 2; }
"#;
    let theirs = r#"function first() { return 1; }
function second() { return 2; }
function computeKey(input) { return `${input.kind}:${input.id}`; }
"#;

    let result = entity_merge(base, ours, theirs, "sample.ts");
    assert!(
        !result.is_clean(),
        "same named additions with different bodies must conflict, not duplicate cleanly. Content:\n{}",
        result.content
    );
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].entity_name, "computeKey");
    assert_eq!(result.conflicts[0].kind, ConflictKind::BothAdded);
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
    assert!(!result.is_clean(), "Modify + delete should be a conflict");
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

/// Regression test for issue #36: closing delimiter placed too early when
/// multiple lines are added at end of a JSON file.
#[test]
fn json_multiple_keys_added_at_end() {
    let base = r#"{
  "key.aaa": "aaa",
  "key.bbb": "bbb",
  "key.ccc": "ccc"
}
"#;
    // Feature: modify one value
    let ours = r#"{
  "key.aaa": "aaa",
  "key.bbb": "BBB",
  "key.ccc": "ccc"
}
"#;
    // Main: add 2 keys at the end
    let theirs = r#"{
  "key.aaa": "aaa",
  "key.bbb": "bbb",
  "key.ccc": "ccc",
  "key.xxx": "xxx",
  "key.yyy": "yyy"
}
"#;

    let result = entity_merge(base, ours, theirs, "data.json");
    assert!(
        result.is_clean(),
        "JSON: modify value + add keys at end should auto-resolve. Conflicts: {:?}",
        result.conflicts
    );
    assert!(
        result.content.contains("\"BBB\""),
        "Should keep ours modification"
    );
    assert!(
        result.content.contains("\"key.xxx\""),
        "Should include first added key"
    );
    assert!(
        result.content.contains("\"key.yyy\""),
        "Should include second added key"
    );

    // Closing brace must come after ALL added keys, not just the first
    let brace_pos = result.content.rfind('}').unwrap();
    let yyy_pos = result.content.find("key.yyy").unwrap();
    assert!(
        brace_pos > yyy_pos,
        "Closing brace must come after key.yyy. Got:\n{}",
        result.content
    );

    // Result should be valid JSON structure (no orphaned lines after closing brace)
    let after_brace = result.content[brace_pos + 1..].trim();
    assert!(
        after_brace.is_empty(),
        "No content should appear after closing brace. Got: '{}'",
        after_brace
    );
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
    assert!(
        result.content.contains("validate"),
        "Should contain ours import"
    );
    assert!(
        result.content.contains("format"),
        "Should contain theirs import"
    );
    assert!(
        result.content.contains("config"),
        "Should keep base imports"
    );
    assert!(
        result.content.contains("logger"),
        "Should keep base imports"
    );
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
    assert!(
        result.content.contains("HashMap"),
        "Should contain theirs use"
    );
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
    assert!(
        result.content.contains("json"),
        "Should contain ours import"
    );
    assert!(
        result.content.contains("pathlib"),
        "Should contain theirs import"
    );
}

// =============================================================================
// Inner entity merge (LastMerge: unordered class members)
// =============================================================================

#[test]
fn ts_class_different_methods_modified_auto_resolves() {
    // THE key multi-agent scenario: two agents modify different methods in the same class
    let base = r#"export class UserService {
    getUser(id: string): User {
        return this.db.find(id);
    }

    createUser(data: UserData): User {
        return this.db.create(data);
    }

    deleteUser(id: string): void {
        this.db.delete(id);
    }
}
"#;
    // Agent A adds caching to getUser
    let ours = r#"export class UserService {
    getUser(id: string): User {
        const cached = this.cache.get(id);
        if (cached) return cached;
        return this.db.find(id);
    }

    createUser(data: UserData): User {
        return this.db.create(data);
    }

    deleteUser(id: string): void {
        this.db.delete(id);
    }
}
"#;
    // Agent B adds validation to createUser
    let theirs = r#"export class UserService {
    getUser(id: string): User {
        return this.db.find(id);
    }

    createUser(data: UserData): User {
        if (!data.email) throw new Error("email required");
        return this.db.create(data);
    }

    deleteUser(id: string): void {
        this.db.delete(id);
    }
}
"#;
    let result = entity_merge(base, ours, theirs, "user-service.ts");
    assert!(
        result.is_clean(),
        "Different class methods modified by different agents should auto-merge. Conflicts: {:?}",
        result.conflicts,
    );
    assert!(
        result.content.contains("cache.get"),
        "Should contain ours's caching change"
    );
    assert!(
        result.content.contains("email required"),
        "Should contain theirs's validation change"
    );
    assert!(
        result.content.contains("deleteUser"),
        "Should preserve unchanged method"
    );
}

#[test]
fn ts_class_one_adds_method_other_modifies_existing() {
    let base = r#"export class Calculator {
    add(a: number, b: number): number {
        return a + b;
    }
}
"#;
    // Agent A modifies existing method
    let ours = r#"export class Calculator {
    add(a: number, b: number): number {
        console.log("add called");
        return a + b;
    }
}
"#;
    // Agent B adds new method
    let theirs = r#"export class Calculator {
    add(a: number, b: number): number {
        return a + b;
    }

    multiply(a: number, b: number): number {
        return a * b;
    }
}
"#;
    let result = entity_merge(base, ours, theirs, "calc.ts");
    assert!(
        result.is_clean(),
        "One modifying, other adding should auto-merge. Conflicts: {:?}",
        result.conflicts,
    );
    assert!(
        result.content.contains("console.log"),
        "Should contain modified add"
    );
    assert!(
        result.content.contains("multiply"),
        "Should contain new method"
    );
}

// =============================================================================
// Rename detection (RefFilter / IntelliMerge-inspired)
// =============================================================================

#[test]
fn ts_one_renames_other_modifies_different_function() {
    // Agent A renames greet → sayHello, Agent B modifies farewell
    let base = r#"export function greet(name: string): string {
    return `Hello, ${name}!`;
}

export function farewell(name: string): string {
    return `Goodbye, ${name}!`;
}
"#;
    // Agent A renames greet to sayHello (same body)
    let ours = r#"export function sayHello(name: string): string {
    return `Hello, ${name}!`;
}

export function farewell(name: string): string {
    return `Goodbye, ${name}!`;
}
"#;
    // Agent B modifies farewell
    let theirs = r#"export function greet(name: string): string {
    return `Hello, ${name}!`;
}

export function farewell(name: string): string {
    console.log("farewell called");
    return `Goodbye, ${name}! See you later.`;
}
"#;
    let result = entity_merge(base, ours, theirs, "greetings.ts");
    assert!(
        result.is_clean(),
        "Rename in one branch + modify in other should auto-resolve. Conflicts: {:?}",
        result.conflicts,
    );
    // Should have the renamed function
    assert!(
        result.content.contains("sayHello"),
        "Should have renamed function"
    );
    // Should have the modified farewell
    assert!(
        result.content.contains("See you later"),
        "Should have modified farewell"
    );
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
fn empty_base_json_both_add_different_keys() {
    // Regression test for https://github.com/Ataraxy-Labs/weave/issues/51
    // Empty base + both sides add different JSON keys should produce
    // conflict markers (exit 1), not silently invalid JSON (exit 0).
    let base = "";
    let ours = "{\n  \"a\": \"1\"\n}\n";
    let theirs = "{\n  \"b\": \"2\"\n}\n";

    let result = entity_merge(base, ours, theirs, "config.json");
    assert!(
        !result.is_clean(),
        "Empty base with different JSON content should conflict, not produce invalid output"
    );
    // The merged output must not contain content after the closing brace
    assert!(
        !result.content.contains("}\n\""),
        "Must not append content after closing brace: {}",
        result.content
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

#[test]
fn ts_class_entity_extraction_includes_child_methods() {
    // sem-core extracts a class AND its methods as child entities.
    // filter_nested_entities() reduces to just the class for top-level matching,
    // but inner entity merge uses the child entities for tree-sitter-accurate
    // method decomposition.
    let ts_class = r#"export class Calculator {
    add(a: number, b: number): number {
        return a + b;
    }

    subtract(a: number, b: number): number {
        return a - b;
    }
}
"#;
    let registry = sem_core::parser::plugins::create_default_registry();
    let plugin = registry.get_plugin("test.ts").unwrap();
    let entities = plugin.extract_entities(ts_class, "test.ts");

    assert_eq!(entities.len(), 3, "Should have class + 2 child methods");
    assert_eq!(entities[0].entity_type, "class");
    assert_eq!(entities[0].name, "Calculator");

    let add = entities.iter().find(|e| e.name == "add").unwrap();
    assert_eq!(add.entity_type, "method");
    assert!(add.parent_id.is_some(), "add should have parent_id");

    let sub = entities.iter().find(|e| e.name == "subtract").unwrap();
    assert_eq!(sub.entity_type, "method");
    assert!(sub.parent_id.is_some(), "subtract should have parent_id");
}

#[test]
fn ts_class_4methods_different_agents_modify_different_methods() {
    // Reproducing exact bench scenario #2 — 4-method class
    let base = r#"export class UserService {
    getUser(id: string): User {
        return this.db.find(id);
    }

    createUser(data: UserData): User {
        return this.db.create(data);
    }

    deleteUser(id: string): void {
        this.db.delete(id);
    }

    listUsers(): User[] {
        return this.db.findAll();
    }
}
"#;
    let ours = r#"export class UserService {
    getUser(id: string): User {
        const cached = this.cache.get(id);
        if (cached) return cached;
        const user = this.db.find(id);
        this.cache.set(id, user);
        return user;
    }

    createUser(data: UserData): User {
        return this.db.create(data);
    }

    deleteUser(id: string): void {
        this.db.delete(id);
    }

    listUsers(): User[] {
        return this.db.findAll();
    }
}
"#;
    let theirs = r#"export class UserService {
    getUser(id: string): User {
        return this.db.find(id);
    }

    createUser(data: UserData): User {
        if (!data.email) throw new Error("email required");
        if (!data.name) throw new Error("name required");
        const user = this.db.create(data);
        this.events.emit("user.created", user);
        return user;
    }

    deleteUser(id: string): void {
        this.db.delete(id);
    }

    listUsers(): User[] {
        return this.db.findAll();
    }
}
"#;
    let result = entity_merge(base, ours, theirs, "service.ts");
    eprintln!("Stats: {:?}", result.stats);
    if !result.is_clean() {
        eprintln!("Conflicts: {:?}", result.conflicts);
        eprintln!("Content:\n{}", result.content);
    }
    assert!(
        result.is_clean(),
        "4-method class: different methods modified should auto-merge. Conflicts: {:?}",
        result.conflicts,
    );
    assert!(result.content.contains("cache.get"));
    assert!(result.content.contains("email required"));
    assert!(result.content.contains("deleteUser"));
    assert!(result.content.contains("listUsers"));
}

#[test]
fn py_class_different_methods_modified_auto_resolves() {
    // Python class: two agents modify different methods (adjacent changes that diffy may fail on)
    let base = r#"class Service:
    def create(self, data):
        return self.db.insert(data)

    def read(self, id):
        return self.db.find(id)

    def update(self, id, data):
        self.db.update(id, data)

    def delete(self, id):
        self.db.remove(id)
"#;
    // Agent A adds validation + logging to create
    let ours = r#"class Service:
    def create(self, data):
        if not data:
            raise ValueError("empty")
        result = self.db.insert(data)
        self.log.info(f"Created {result.id}")
        return result

    def read(self, id):
        return self.db.find(id)

    def update(self, id, data):
        self.db.update(id, data)

    def delete(self, id):
        self.db.remove(id)
"#;
    // Agent B adds caching to read
    let theirs = r#"class Service:
    def create(self, data):
        return self.db.insert(data)

    def read(self, id):
        cached = self.cache.get(id)
        if cached:
            return cached
        result = self.db.find(id)
        self.cache.set(id, result)
        return result

    def update(self, id, data):
        self.db.update(id, data)

    def delete(self, id):
        self.db.remove(id)
"#;
    let result = entity_merge(base, ours, theirs, "service.py");
    assert!(
        result.is_clean(),
        "Python class: different methods modified should auto-merge. Conflicts: {:?}",
        result.conflicts,
    );
    assert!(
        result.content.contains("raise ValueError"),
        "Should contain ours's validation"
    );
    assert!(
        result.content.contains("self.cache.get"),
        "Should contain theirs's caching"
    );
    assert!(
        result.content.contains("def update"),
        "Should preserve unchanged methods"
    );
}

#[test]
fn ts_one_reformats_other_modifies_no_conflict() {
    // Agent A reformats indentation, Agent B makes semantic change
    // Should detect that A's changes are whitespace-only and take B's version
    let base = r#"export function process(data: string): string {
    return data.trim();
}
"#;
    // Agent A only changes whitespace (adds extra indentation)
    let ours = r#"export function process(data: string): string {
      return data.trim();
}
"#;
    // Agent B makes a real change
    let theirs = r#"export function process(data: string): string {
    const cleaned = data.trim();
    console.log("Processing:", cleaned);
    return cleaned.toUpperCase();
}
"#;
    let result = entity_merge(base, ours, theirs, "utils.ts");
    assert!(
        result.is_clean(),
        "Whitespace-only change vs real change should not conflict. Conflicts: {:?}",
        result.conflicts,
    );
    assert!(
        result.content.contains("toUpperCase"),
        "Should take the real change (theirs)"
    );
}

#[test]
fn ts_both_reformat_same_function_no_conflict() {
    // Both agents only change whitespace — should resolve cleanly
    let base = r#"export function hello(): string {
    return "hello";
}
"#;
    let ours = r#"export function hello(): string {
      return "hello";
}
"#;
    let theirs = r#"export function hello(): string {
        return "hello";
}
"#;
    let result = entity_merge(base, ours, theirs, "fmt.ts");
    assert!(
        result.is_clean(),
        "Both whitespace-only changes should not conflict. Conflicts: {:?}",
        result.conflicts,
    );
}

// =============================================================================
// Java: method-level merge and annotation merge
// =============================================================================

#[test]
fn java_different_methods_modified_auto_resolves() {
    let base = r#"public class UserService {
    public User getUser(String id) {
        return db.find(id);
    }

    public void createUser(User user) {
        db.save(user);
    }
}
"#;
    let ours = r#"public class UserService {
    public User getUser(String id) {
        User user = db.find(id);
        logger.info("Found: " + id);
        return user;
    }

    public void createUser(User user) {
        db.save(user);
    }
}
"#;
    let theirs = r#"public class UserService {
    public User getUser(String id) {
        return db.find(id);
    }

    public void createUser(User user) {
        validateUser(user);
        db.save(user);
    }
}
"#;
    let result = entity_merge(base, ours, theirs, "UserService.java");
    assert!(
        result.is_clean(),
        "Different Java methods modified should auto-resolve. Conflicts: {:?}",
        result.conflicts,
    );
    assert!(
        result.content.contains("logger.info"),
        "Should contain ours change"
    );
    assert!(
        result.content.contains("validateUser"),
        "Should contain theirs change"
    );
}

#[test]
fn java_both_add_different_annotations() {
    let base = r#"public class Controller {
    public Response handle(Request req) {
        return service.process(req);
    }
}
"#;
    let ours = r#"public class Controller {
    @Cacheable(ttl = 60)
    public Response handle(Request req) {
        return service.process(req);
    }
}
"#;
    let theirs = r#"public class Controller {
    @RateLimit(100)
    public Response handle(Request req) {
        return service.process(req);
    }
}
"#;
    let result = entity_merge(base, ours, theirs, "Controller.java");
    assert!(
        result.is_clean(),
        "Both adding different annotations should auto-resolve. Conflicts: {:?}",
        result.conflicts,
    );
    assert!(
        result.content.contains("@Cacheable"),
        "Should contain ours annotation"
    );
    assert!(
        result.content.contains("@RateLimit"),
        "Should contain theirs annotation"
    );
}

// =============================================================================
// C: function-level merge
// =============================================================================

#[test]
fn c_different_functions_modified_auto_resolves() {
    let base = r#"void init(Config* cfg) {
    cfg->ready = 1;
}

int process(Data* data) {
    return data->value * 2;
}
"#;
    let ours = r#"void init(Config* cfg) {
    cfg->ready = 1;
    log_debug("initialized");
}

int process(Data* data) {
    return data->value * 2;
}
"#;
    let theirs = r#"void init(Config* cfg) {
    cfg->ready = 1;
}

int process(Data* data) {
    if (data == NULL) return -1;
    return data->value * 2;
}
"#;
    let result = entity_merge(base, ours, theirs, "utils.c");
    assert!(
        result.is_clean(),
        "Different C functions modified should auto-resolve. Conflicts: {:?}",
        result.conflicts,
    );
    assert!(
        result.content.contains("log_debug"),
        "Should contain ours change"
    );
    assert!(
        result.content.contains("NULL"),
        "Should contain theirs change"
    );
}

// =============================================================================
// Method reordering
// =============================================================================

#[test]
fn ts_method_reorder_plus_modification_auto_resolves() {
    // Agent A reorders methods, Agent B modifies a method
    let base = r#"class Service {
    getUser(id: string) {
        return db.find(id);
    }

    createUser(data: any) {
        return db.create(data);
    }

    deleteUser(id: string) {
        return db.delete(id);
    }
}
"#;
    let ours = r#"class Service {
    getUser(id: string) {
        return db.find(id);
    }

    deleteUser(id: string) {
        return db.delete(id);
    }

    createUser(data: any) {
        return db.create(data);
    }
}
"#;
    let theirs = r#"class Service {
    getUser(id: string) {
        console.log("fetching", id);
        return db.find(id);
    }

    createUser(data: any) {
        return db.create(data);
    }

    deleteUser(id: string) {
        return db.delete(id);
    }
}
"#;
    let result = entity_merge(base, ours, theirs, "service.ts");
    assert!(
        result.is_clean(),
        "Method reorder + modification should auto-resolve. Conflicts: {:?}",
        result.conflicts,
    );
    assert!(
        result.content.contains("console.log(\"fetching\""),
        "Should have theirs modification"
    );
    assert!(
        result.content.contains("deleteUser"),
        "Should have all methods"
    );
    assert!(
        result.content.contains("createUser"),
        "Should have all methods"
    );
}

// =============================================================================
// Python class inner entity merge
// =============================================================================

#[test]
fn python_class_both_add_methods_auto_resolves() {
    let base = "class Calculator:\n    def add(self, a, b):\n        return a + b\n";
    let ours = "class Calculator:\n    def add(self, a, b):\n        return a + b\n\n    def multiply(self, a, b):\n        return a * b\n";
    let theirs = "class Calculator:\n    def add(self, a, b):\n        return a + b\n\n    def divide(self, a, b):\n        return a / b\n";
    let result = entity_merge(base, ours, theirs, "calculator.py");
    assert!(
        result.is_clean(),
        "Both adding methods to Python class should auto-resolve. Conflicts: {:?}",
        result.conflicts,
    );
    assert!(
        result.content.contains("multiply"),
        "Should have ours method"
    );
    assert!(
        result.content.contains("divide"),
        "Should have theirs method"
    );
}

// =============================================================================
// Rust impl block merge
// =============================================================================

#[test]
fn rust_impl_both_add_methods_auto_resolves() {
    let base = r#"impl Calculator {
    fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}
"#;
    let ours = r#"impl Calculator {
    fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }
}
"#;
    let theirs = r#"impl Calculator {
    fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    fn divide(&self, a: i32, b: i32) -> i32 {
        a / b
    }
}
"#;
    let result = entity_merge(base, ours, theirs, "calc.rs");
    assert!(
        result.is_clean(),
        "Both adding methods to Rust impl should auto-resolve. Conflicts: {:?}",
        result.conflicts,
    );
    assert!(
        result.content.contains("multiply"),
        "Should have ours method"
    );
    assert!(
        result.content.contains("divide"),
        "Should have theirs method"
    );
}

// =============================================================================
// Go: both add functions
// =============================================================================

#[test]
fn go_both_add_different_functions_auto_resolves() {
    let base = r#"package handlers

func HandleGet(w http.ResponseWriter, r *http.Request) {
    w.WriteHeader(http.StatusOK)
}
"#;
    let ours = r#"package handlers

func HandleGet(w http.ResponseWriter, r *http.Request) {
    w.WriteHeader(http.StatusOK)
}

func HandlePost(w http.ResponseWriter, r *http.Request) {
    w.WriteHeader(http.StatusCreated)
}
"#;
    let theirs = r#"package handlers

func HandleGet(w http.ResponseWriter, r *http.Request) {
    w.WriteHeader(http.StatusOK)
}

func HandleDelete(w http.ResponseWriter, r *http.Request) {
    w.WriteHeader(http.StatusNoContent)
}
"#;
    let result = entity_merge(base, ours, theirs, "handlers.go");
    assert!(
        result.is_clean(),
        "Both adding Go functions should auto-resolve. Conflicts: {:?}",
        result.conflicts,
    );
    assert!(
        result.content.contains("HandlePost"),
        "Should have ours function"
    );
    assert!(
        result.content.contains("HandleDelete"),
        "Should have theirs function"
    );
}

// =============================================================================
// Enum variant modify + add
// =============================================================================

#[test]
fn ts_enum_modify_variant_plus_add_variant_auto_resolves() {
    let base = "enum Status {\n    Active = \"active\",\n    Inactive = \"inactive\",\n    Pending = \"pending\",\n}\n";
    let ours = "enum Status {\n    Active = \"active\",\n    Inactive = \"disabled\",\n    Pending = \"pending\",\n}\n";
    let theirs = "enum Status {\n    Active = \"active\",\n    Inactive = \"inactive\",\n    Pending = \"pending\",\n    Deleted = \"deleted\",\n}\n";
    let result = entity_merge(base, ours, theirs, "status.ts");
    assert!(
        result.is_clean(),
        "Enum modify + add should auto-resolve. Conflicts: {:?}",
        result.conflicts,
    );
    assert!(
        result.content.contains("\"disabled\""),
        "Should have modified variant"
    );
    assert!(
        result.content.contains("Deleted"),
        "Should have new variant"
    );
}

// =============================================================================
// Rename/rename conflict: both branches rename the same entity to different names
// =============================================================================

#[test]
fn rust_rename_rename_conflict_detected() {
    let base = r#"#[derive(Debug, Clone)]
pub enum Source {
    Api,
    File,
    Manual,
}
"#;
    let ours = r#"#[derive(Debug, Clone)]
pub enum Source1 {
    Api,
    File,
    Manual,
}
"#;
    let theirs = r#"#[derive(Debug, Clone)]
pub enum BSource {
    Api,
    File,
    Manual,
}
"#;
    let result = entity_merge(base, ours, theirs, "types.rs");
    assert!(
        !result.is_clean(),
        "Both branches renaming the same entity should be a conflict, not silently keep both"
    );
    assert_eq!(result.conflicts.len(), 1);
    let conflict = &result.conflicts[0];
    assert!(
        format!("{}", conflict.kind).contains("both renamed"),
        "Should be a rename/rename conflict, got: {}",
        conflict.kind
    );
}

#[test]
fn rust_rename_rename_multi_entity_file() {
    let base = r#"use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum Source {
    Api,
    File,
    Manual,
}

pub fn process() -> String {
    "hello".to_string()
}

pub struct Config {
    pub name: String,
}
"#;
    let ours = r#"use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum Source1 {
    Api,
    File,
    Manual,
}

pub fn process() -> String {
    "hello".to_string()
}

pub struct Config {
    pub name: String,
}
"#;
    let theirs = r#"use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum BSource {
    Api,
    File,
    Manual,
}

pub fn process() -> String {
    "hello".to_string()
}

pub struct Config {
    pub name: String,
}
"#;
    let result = entity_merge(base, ours, theirs, "types.rs");
    eprintln!("content:\n{}", result.content);
    eprintln!("conflicts: {:?}", result.conflicts.len());
    for c in &result.conflicts {
        eprintln!("  conflict: {} - {}", c.entity_name, c.kind);
    }
    assert!(
        !result.is_clean(),
        "Rename-rename in multi-entity file should conflict, got clean merge:\n{}",
        result.content
    );
}

// Slarse bug: inner entity merge should scope conflicts to the individual method,
// not wrap the entire class in conflict markers.
#[test]
fn java_class_conflict_scoped_to_method() {
    let base = r#"public class Main {
    public int add(int a, int b) {
        return a + b;
    }

    public int subtract(int a, int b) {
        return a - b;
    }
}
"#;
    let ours = r#"public class Main {
    public int add(int a, int b) throws IllegalArgumentException {
        return a + b;
    }

    public int subtract(int a, int b) {
        return a - b;
    }
}
"#;
    let theirs = r#"public class Main {
    public int add(int a, int b, int c) {
        return a + b + c;
    }

    public int subtract(int a, int b) {
        return a - b;
    }
}
"#;
    let result = entity_merge(base, ours, theirs, "Main.java");
    eprintln!("content:\n{}", result.content);
    eprintln!("conflicts: {}", result.conflicts.len());
    for c in &result.conflicts {
        eprintln!("  conflict: {} - {}", c.entity_name, c.kind);
    }

    // The conflict should exist (both modified same method)
    assert!(
        !result.is_clean(),
        "Should have a conflict on the add method"
    );

    // The subtract method should NOT be inside conflict markers
    // (it was not modified by either branch)
    let content = &result.content;
    assert!(
        !content.contains("subtract")
            || !is_inside_conflict_markers(content, "subtract"),
        "subtract() should not be inside conflict markers - conflict should be scoped to add() only"
    );
}

// =============================================================================
// Slarse scenarios: verify scoped conflicts across languages
// =============================================================================

#[test]
fn java_throws_vs_param_change() {
    // Slarse's exact scenario: one adds throws, other changes params+body
    let base = r#"public class Main {
    public int add(int a, int b) {
        return a + b;
    }

    public int subtract(int a, int b) {
        return a - b;
    }
}"#;
    let ours = r#"public class Main {
    public int add(int a, int b) throws IllegalArgumentException {
        return a + b;
    }

    public int subtract(int a, int b) {
        return a - b;
    }
}"#;
    let theirs = r#"public class Main {
    public int add(int a, int b, int c) {
        return a + b + c;
    }

    public int subtract(int a, int b) {
        return a - b;
    }
}"#;
    let result = entity_merge(base, ours, theirs, "Main.java");
    eprintln!("--- slarse java throws vs param ---");
    eprintln!("content:\n{}", result.content);
    eprintln!("conflicts: {}", result.conflicts.len());

    assert!(!result.is_clean(), "Should conflict on add()");
    assert!(
        !is_inside_conflict_markers(&result.content, "subtract"),
        "subtract should NOT be inside conflict markers"
    );
    // Verify subtract appears cleanly
    assert!(
        result.content.contains("public int subtract"),
        "subtract should be in output"
    );
}

#[test]
fn java_param_vs_annotation() {
    // One adds param, other adds annotation
    let base = r#"public class Service {
    public User getUser(String id) {
        return db.find(id);
    }

    public void deleteUser(String id) {
        db.remove(id);
    }
}"#;
    let ours = r#"public class Service {
    public User getUser(String id, boolean includeDeleted) {
        return db.find(id);
    }

    public void deleteUser(String id) {
        db.remove(id);
    }
}"#;
    let theirs = r#"public class Service {
    @Cacheable
    public User getUser(String id) {
        return db.find(id);
    }

    public void deleteUser(String id) {
        db.remove(id);
    }
}"#;
    let result = entity_merge(base, ours, theirs, "Service.java");
    eprintln!("--- java param vs annotation ---");
    eprintln!("content:\n{}", result.content);
    eprintln!(
        "clean: {}, conflicts: {}",
        result.is_clean(),
        result.conflicts.len()
    );

    // deleteUser should never be in conflict
    assert!(
        !is_inside_conflict_markers(&result.content, "deleteUser"),
        "deleteUser should NOT be inside conflict markers"
    );
}

#[test]
fn java_large_class_one_conflict() {
    // Large class, only one method conflicted, rest should be clean
    let base = r#"public class BigService {
    public void methodA() {
        System.out.println("A");
    }

    public void methodB() {
        System.out.println("B");
    }

    public void methodC() {
        System.out.println("C");
    }

    public void target() {
        System.out.println("original");
    }
}"#;
    let ours = r#"public class BigService {
    public void methodA() {
        System.out.println("A");
    }

    public void methodB() {
        System.out.println("B");
    }

    public void methodC() {
        System.out.println("C");
    }

    public void target() {
        System.out.println("ours version");
    }
}"#;
    let theirs = r#"public class BigService {
    public void methodA() {
        System.out.println("A");
    }

    public void methodB() {
        System.out.println("B");
    }

    public void methodC() {
        System.out.println("C");
    }

    public void target() {
        System.out.println("theirs version");
    }
}"#;
    let result = entity_merge(base, ours, theirs, "BigService.java");
    eprintln!("--- large class one conflict ---");
    eprintln!("content:\n{}", result.content);

    assert!(!result.is_clean(), "Should conflict on target()");
    assert_eq!(result.conflicts.len(), 1, "Should be exactly 1 conflict");
    for method in &["methodA", "methodB", "methodC"] {
        assert!(
            !is_inside_conflict_markers(&result.content, method),
            "{} should NOT be inside conflict markers",
            method
        );
        assert!(
            result.content.contains(method),
            "{} should be in output",
            method
        );
    }
}

#[test]
fn ts_class_scoped_conflict() {
    // TS version: same member scoping
    let base = r#"export class UserService {
    getUser(id: string): User {
        return this.db.find(id);
    }

    createUser(data: UserData): User {
        return this.db.create(data);
    }

    deleteUser(id: string): void {
        this.db.delete(id);
    }
}"#;
    let ours = r#"export class UserService {
    getUser(id: string): User {
        return this.db.findOne(id);
    }

    createUser(data: UserData): User {
        return this.db.create(data);
    }

    deleteUser(id: string): void {
        this.db.delete(id);
    }
}"#;
    let theirs = r#"export class UserService {
    getUser(id: string): User {
        const cached = this.cache.get(id);
        return cached || this.db.find(id);
    }

    createUser(data: UserData): User {
        return this.db.create(data);
    }

    deleteUser(id: string): void {
        this.db.delete(id);
    }
}"#;
    let result = entity_merge(base, ours, theirs, "UserService.ts");
    eprintln!("--- ts class scoped conflict ---");
    eprintln!("content:\n{}", result.content);

    assert!(!result.is_clean(), "Should conflict on getUser");
    for method in &["createUser", "deleteUser"] {
        assert!(
            !is_inside_conflict_markers(&result.content, method),
            "{} should NOT be inside conflict markers",
            method
        );
    }
}

#[test]
fn python_class_scoped_conflict() {
    let base = r#"class Service:
    def create(self, data):
        return self.db.insert(data)

    def read(self, id):
        return self.db.find(id)

    def delete(self, id):
        self.db.remove(id)
"#;
    let ours = r#"class Service:
    def create(self, data):
        return self.db.insert(data)

    def read(self, id):
        return self.db.find_one(id)

    def delete(self, id):
        self.db.remove(id)
"#;
    let theirs = r#"class Service:
    def create(self, data):
        return self.db.insert(data)

    def read(self, id):
        cached = self.cache.get(id)
        if cached:
            return cached
        return self.db.find(id)

    def delete(self, id):
        self.db.remove(id)
"#;
    let result = entity_merge(base, ours, theirs, "service.py");
    eprintln!("--- python class scoped conflict ---");
    eprintln!("content:\n{}", result.content);

    assert!(!result.is_clean(), "Should conflict on read");
    for method in &["def create", "def delete"] {
        assert!(
            !is_inside_conflict_markers(&result.content, method),
            "{} should NOT be inside conflict markers",
            method
        );
    }
}

#[test]
fn rust_impl_scoped_conflict() {
    let base = r#"impl Server {
    fn handle_get(&self, req: Request) -> Response {
        Response::ok()
    }

    fn handle_post(&self, req: Request) -> Response {
        Response::created()
    }

    fn handle_delete(&self, req: Request) -> Response {
        Response::no_content()
    }
}"#;
    let ours = r#"impl Server {
    fn handle_get(&self, req: Request) -> Response {
        let data = self.db.get(req.id);
        Response::ok_with(data)
    }

    fn handle_post(&self, req: Request) -> Response {
        Response::created()
    }

    fn handle_delete(&self, req: Request) -> Response {
        Response::no_content()
    }
}"#;
    let theirs = r#"impl Server {
    fn handle_get(&self, req: Request) -> Response {
        self.auth.check(&req)?;
        Response::ok()
    }

    fn handle_post(&self, req: Request) -> Response {
        Response::created()
    }

    fn handle_delete(&self, req: Request) -> Response {
        Response::no_content()
    }
}"#;
    let result = entity_merge(base, ours, theirs, "server.rs");
    eprintln!("--- rust impl scoped conflict ---");
    eprintln!("content:\n{}", result.content);

    assert!(!result.is_clean(), "Should conflict on handle_get");
    for method in &["handle_post", "handle_delete"] {
        assert!(
            !is_inside_conflict_markers(&result.content, method),
            "{} should NOT be inside conflict markers",
            method
        );
    }
}

#[test]
fn ts_object_literal_different_properties_added() {
    let base = r#"const config = {
    a: 1,
    c: 3,
};
"#;
    let ours = r#"const config = {
    a: 1,
    b: 2,
    c: 3,
};
"#;
    let theirs = r#"const config = {
    a: 1,
    c: 3,
    d: 4,
};
"#;

    let result = entity_merge(base, ours, theirs, "config.ts");
    assert!(
        result.is_clean(),
        "Adding different properties to an object literal should auto-resolve. Conflicts: {:?}",
        result.conflicts
    );
    assert!(result.content.contains("a: 1"));
    assert!(result.content.contains("b: 2"));
    assert!(result.content.contains("c: 3"));
    assert!(result.content.contains("d: 4"));
}

// Issue #22: function conflict markers should be narrowed to just the differing lines
#[test]
fn ts_function_conflict_narrowed_to_changed_lines() {
    let base = r#"async function foo(example: Example) {
    someFunctionCalls()
    anotherCall()

    return join.lines(
        `Example: "${prompt}".`,
        `Category: ${category?.join(", ")}`,
        includeContext && `Example Context: ${createExampleContextMessage()}`,
        contextImages && `Example Images: <${Images.metadataTag}>${JSON.stringify(contextImages)}</${Images.metadataTag}>`,
        commands ? `Expected Output: ${commands}` : "",
    )
}
"#;
    let ours = r#"async function foo(example: Example) {
    someFunctionCalls()
    anotherCall()

    return join.lines(
        `Example: "${prompt}".`,
        `Category: ${category?.join(", ")}`,
        includeContext && `Example Context: ${sanitize(createExampleContextMessage())}`,
        contextImages && `Example Images: <${Images.metadataTag}>${serialize(contextImages)}</${Images.metadataTag}>`,
        commands ? `Expected Output: ${commands}` : "",
    )
}
"#;
    let theirs = r#"async function foo(example: Example) {
    someFunctionCalls()
    anotherCall()

    return join.lines(
        `Example: "${prompt}".`,
        `Category: ${category?.join(", ")}`,
        includeContext && `Example Context: ${escapeBlock(createExampleContextMessage())}`,
        contextImages && `Example Images: <${Images.metadataTag}>${serializeJSON(contextImages)}</${Images.metadataTag}>`,
        commands ? `Expected Output: ${commands}` : "",
    )
}
"#;
    let result = entity_merge(base, ours, theirs, "test.ts");
    eprintln!("--- narrowed function conflict ---");
    eprintln!("content:\n{}", result.content);

    assert!(!result.is_clean(), "Should conflict on the changed lines");
    // The unchanged lines should NOT be inside conflict markers
    assert!(
        !is_inside_conflict_markers(&result.content, "someFunctionCalls"),
        "Unchanged lines like someFunctionCalls() should be outside conflict markers"
    );
    assert!(
        !is_inside_conflict_markers(&result.content, "anotherCall"),
        "Unchanged lines like anotherCall() should be outside conflict markers"
    );
    assert!(
        !is_inside_conflict_markers(&result.content, "Expected Output"),
        "Unchanged lines like Expected Output should be outside conflict markers"
    );
}

// =============================================================================
// Scala
// =============================================================================

#[test]
fn scala_two_agents_add_different_methods() {
    let base = r#"class UserService {
  def findById(id: String): Option[User] = db.find(id)
}
"#;
    let ours = r#"class UserService {
  def findById(id: String): Option[User] = db.find(id)

  def create(user: User): User = db.save(user)
}
"#;
    let theirs = r#"class UserService {
  def findById(id: String): Option[User] = db.find(id)

  def delete(id: String): Unit = db.remove(id)
}
"#;

    let result = entity_merge(base, ours, theirs, "UserService.scala");
    assert!(
        result.is_clean(),
        "Two agents adding different methods should auto-resolve. Conflicts: {:?}",
        result.conflicts
    );
    assert!(result.content.contains("create"));
    assert!(result.content.contains("delete"));
    assert!(result.content.contains("findById"));
}

#[test]
fn scala_one_modifies_one_adds() {
    // Top-level definitions (Scala 3 style) — one side modifies, other adds
    let base = r#"package com.example

def greet(name: String): String = s"Hello, $name"
"#;
    let ours = r#"package com.example

def greet(name: String): String = s"Hello, $name!"
"#;
    let theirs = r#"package com.example

def greet(name: String): String = s"Hello, $name"

def farewell(name: String): String = s"Goodbye, $name"
"#;

    let result = entity_merge(base, ours, theirs, "greetings.scala");
    assert!(
        result.is_clean(),
        "One modifying, one adding should auto-resolve. Conflicts: {:?}",
        result.conflicts
    );
    assert!(result.content.contains("Hello, $name!"));
    assert!(result.content.contains("farewell"));
}

#[test]
fn scala_both_modify_same_method_incompatibly() {
    // Top-level definition (Scala 3 style) — both modify incompatibly
    let base = r#"package com.example

def process(data: String): String = data.trim()
"#;
    let ours = r#"package com.example

def process(data: String): String = data.trim().toUpperCase()
"#;
    let theirs = r#"package com.example

def process(data: String): String = data.trim().toLowerCase()
"#;

    let result = entity_merge(base, ours, theirs, "processor.scala");
    assert!(!result.is_clean());
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].entity_name, "process");
}

#[test]
fn scala_add_different_top_level_definitions() {
    let base = r#"package com.example

trait Repository[T] {
  def findAll(): List[T]
}
"#;
    let ours = r#"package com.example

trait Repository[T] {
  def findAll(): List[T]
}

case class User(id: String, name: String)
"#;
    let theirs = r#"package com.example

trait Repository[T] {
  def findAll(): List[T]
}

case class Product(id: String, price: Double)
"#;

    let result = entity_merge(base, ours, theirs, "models.scala");
    assert!(
        result.is_clean(),
        "Adding different top-level case classes should auto-resolve. Conflicts: {:?}",
        result.conflicts
    );
    assert!(result.content.contains("User"));
    assert!(result.content.contains("Product"));
}

// =============================================================================
// Dart
// =============================================================================

#[test]
fn dart_two_agents_add_different_functions() {
    let base = r#"class Calculator {
  int value = 0;
}
"#;
    let ours = r#"class Calculator {
  int value = 0;

  void add(int amount) {
    value += amount;
  }
}
"#;
    let theirs = r#"class Calculator {
  int value = 0;

  void subtract(int amount) {
    value -= amount;
  }
}
"#;

    let result = entity_merge(base, ours, theirs, "calculator.dart");
    assert!(
        result.is_clean(),
        "Two agents adding different functions to Dart class should auto-resolve. Conflicts: {:?}",
        result.conflicts
    );
    assert!(result.content.contains("add(int amount)"));
    assert!(result.content.contains("subtract(int amount)"));
}

#[test]
fn dart_one_modifies_one_adds() {
    let base = r#"void greet(String name) {
  print('Hello, $name');
}
"#;
    let ours = r#"void greet(String name) {
  print('Greetings, $name!');
}
"#;
    let theirs = r#"void greet(String name) {
  print('Hello, $name');
}

void sayGoodbye(String name) {
  print('Goodbye, $name');
}
"#;

    let result = entity_merge(base, ours, theirs, "greetings.dart");
    assert!(
        result.is_clean(),
        "One modifies existing function, other adds new function should auto-resolve. Conflicts: {:?}",
        result.conflicts
    );
    assert!(result.content.contains("Greetings, $name!"));
    assert!(result.content.contains("sayGoodbye"));
}

#[test]
fn dart_both_modify_same_method_incompatibly() {
    let base = r#"void process(String data) {
  var d = data;
  print(d);
}
"#;
    let ours = r#"void process(String data) {
  var d = data.toLowerCase();
  print(d);
}
"#;
    let theirs = r#"void process(String data) {
  var d = data.toUpperCase();
  print(d);
}
"#;

    let result = entity_merge(base, ours, theirs, "processor.dart");
    assert!(
        !result.is_clean(),
        "Incompatible changes to the same function must conflict"
    );
    assert!(is_inside_conflict_markers(&result.content, "toLowerCase()"));
    assert!(is_inside_conflict_markers(&result.content, "toUpperCase()"));
}

// =============================================================================
// Import & file-header preservation scenarios (#94, #95)
// =============================================================================

/// Scenario 1: theirs adds import + edits type, ours also edits type (issue #95 repro)
#[test]
fn ts_import_preserved_both_edit_type() {
    let base = r#"type Config = {
  host: string;
  port: number;
};
"#;
    let ours = r#"type Config = {
  host: string;
  port: number;
  timeout: number;
};
"#;
    let theirs = r#"import { Env } from './env';

type Config = {
  host: string;
  port: Env;
};
"#;
    let result = entity_merge(base, ours, theirs, "config.ts");
    assert!(
        result.content.contains("import { Env }"),
        "Import added by theirs must survive when both edit the type.\nGot:\n{}",
        result.content
    );
    assert!(
        result.content.contains("timeout"),
        "Field added by ours must be present.\nGot:\n{}",
        result.content
    );
}

/// Scenario 2: both branches add different imports, no entity changes
#[test]
fn ts_both_add_imports_no_entity_change() {
    let base = r#"import { a } from './a';

export function greet() {
    return "hello";
}
"#;
    let ours = r#"import { a } from './a';
import { b } from './b';

export function greet() {
    return "hello";
}
"#;
    let theirs = r#"import { a } from './a';
import { c } from './c';

export function greet() {
    return "hello";
}
"#;
    let result = entity_merge(base, ours, theirs, "greet.ts");
    assert!(
        result.is_clean(),
        "Should merge cleanly. Conflicts: {:?}",
        result.conflicts
    );
    assert!(
        result.content.contains("import { a }"),
        "Original import missing"
    );
    assert!(
        result.content.contains("import { b }"),
        "Ours import missing"
    );
    assert!(
        result.content.contains("import { c }"),
        "Theirs import missing"
    );
    assert!(result.content.contains("greet"), "Function must survive");
}

/// Scenario 3: // @ts-nocheck preserved when both add imports (issue #94 repro)
#[test]
fn ts_nocheck_stays_above_imports_both_add() {
    let base = r#"// @ts-nocheck
import { a } from './a';

export function run() {}
"#;
    let ours = r#"// @ts-nocheck
import { a } from './a';
import { b } from './b';

export function run() {}
"#;
    let theirs = r#"// @ts-nocheck
import { a } from './a';
import { c } from './c';

export function run() {}
"#;
    let result = entity_merge(base, ours, theirs, "run.ts");
    assert!(
        result.is_clean(),
        "Should merge cleanly. Conflicts: {:?}",
        result.conflicts
    );
    let content = &result.content;
    let nocheck = content.find("// @ts-nocheck");
    let first_import = content.find("import");
    assert!(
        nocheck.is_some(),
        "// @ts-nocheck must be present.\n{}",
        content
    );
    assert!(
        nocheck.unwrap() < first_import.unwrap(),
        "// @ts-nocheck must stay before imports.\n{}",
        content
    );
}

/// Scenario 4: eslint-disable directive preserved above imports
#[test]
fn ts_eslint_disable_stays_above_imports() {
    let base = r#"/* eslint-disable */
import { x } from './x';

export function foo() { return 1; }
"#;
    let ours = r#"/* eslint-disable */
import { x } from './x';
import { y } from './y';

export function foo() { return 1; }
"#;
    let theirs = r#"/* eslint-disable */
import { x } from './x';
import { z } from './z';

export function foo() { return 1; }
"#;
    let result = entity_merge(base, ours, theirs, "foo.ts");
    assert!(
        result.is_clean(),
        "Should merge cleanly. Conflicts: {:?}",
        result.conflicts
    );
    let content = &result.content;
    let eslint = content.find("/* eslint-disable */");
    let first_import = content.find("import");
    assert!(
        eslint.is_some(),
        "eslint-disable must be present.\n{}",
        content
    );
    assert!(
        eslint.unwrap() < first_import.unwrap(),
        "eslint-disable must stay before imports.\n{}",
        content
    );
}

/// Scenario 5: theirs adds import, ours adds a new function — clean merge
#[test]
fn ts_theirs_adds_import_ours_adds_function() {
    let base = r#"export function existing() {
    return 42;
}
"#;
    let ours = r#"export function existing() {
    return 42;
}

export function newHelper() {
    return "help";
}
"#;
    let theirs = r#"import { util } from './util';

export function existing() {
    return util(42);
}
"#;
    let result = entity_merge(base, ours, theirs, "module.ts");
    assert!(
        result.content.contains("import { util }"),
        "Import from theirs must be present.\nGot:\n{}",
        result.content
    );
    assert!(
        result.content.contains("newHelper"),
        "Function added by ours must be present.\nGot:\n{}",
        result.content
    );
}

/// Scenario 6: both add imports from different modules + both modify different functions
#[test]
fn ts_both_add_imports_and_modify_different_functions() {
    let base = r#"import { config } from './config';

export function alpha() {
    return config.a;
}

export function beta() {
    return config.b;
}
"#;
    let ours = r#"import { config } from './config';
import { logger } from './logger';

export function alpha() {
    logger.info("alpha called");
    return config.a;
}

export function beta() {
    return config.b;
}
"#;
    let theirs = r#"import { config } from './config';
import { metrics } from './metrics';

export function alpha() {
    return config.a;
}

export function beta() {
    metrics.count("beta");
    return config.b;
}
"#;
    let result = entity_merge(base, ours, theirs, "service.ts");
    assert!(
        result.is_clean(),
        "Should merge cleanly. Conflicts: {:?}",
        result.conflicts
    );
    assert!(
        result.content.contains("import { logger }"),
        "ours import missing"
    );
    assert!(
        result.content.contains("import { metrics }"),
        "theirs import missing"
    );
    assert!(
        result.content.contains("logger.info"),
        "ours change to alpha missing"
    );
    assert!(
        result.content.contains("metrics.count"),
        "theirs change to beta missing"
    );
}

/// Scenario 7: JS require() style — both add different requires
#[test]
fn js_both_add_different_requires() {
    let base = r#"const fs = require('fs');

function readFile(path) {
    return fs.readFileSync(path, 'utf8');
}
"#;
    let ours = r#"const fs = require('fs');
const path = require('path');

function readFile(path) {
    return fs.readFileSync(path, 'utf8');
}
"#;
    let theirs = r#"const fs = require('fs');
const os = require('os');

function readFile(path) {
    return fs.readFileSync(path, 'utf8');
}
"#;
    let result = entity_merge(base, ours, theirs, "file.js");
    assert!(
        result.is_clean(),
        "Should merge cleanly. Conflicts: {:?}",
        result.conflicts
    );
    assert!(result.content.contains("readFile"), "Function must survive");
}

/// Scenario 8: theirs adds multiple imports before first entity, ours untouched
#[test]
fn ts_theirs_adds_multiple_imports_ours_untouched() {
    let base = r#"export function process() {
    return null;
}
"#;
    let ours = r#"export function process() {
    return null;
}
"#;
    let theirs = r#"import { Parser } from './parser';
import { Validator } from './validator';
import { Formatter } from './formatter';

export function process() {
    const p = new Parser();
    const v = new Validator();
    return new Formatter().format(v.validate(p.parse()));
}
"#;
    let result = entity_merge(base, ours, theirs, "pipeline.ts");
    assert!(
        result.is_clean(),
        "Should merge cleanly. Conflicts: {:?}",
        result.conflicts
    );
    assert!(
        result.content.contains("import { Parser }"),
        "Parser import missing"
    );
    assert!(
        result.content.contains("import { Validator }"),
        "Validator import missing"
    );
    assert!(
        result.content.contains("import { Formatter }"),
        "Formatter import missing"
    );
}

/// Scenario 9: 'use strict' directive at top of JS file — must stay on line 1
#[test]
fn js_use_strict_stays_at_top() {
    let base = r#"'use strict';

const { readFileSync } = require('fs');

function load(path) {
    return JSON.parse(readFileSync(path, 'utf8'));
}
"#;
    let ours = r#"'use strict';

const { readFileSync } = require('fs');

function load(path) {
    return JSON.parse(readFileSync(path, 'utf8'));
}

function save(path, data) {
    require('fs').writeFileSync(path, JSON.stringify(data));
}
"#;
    let theirs = r#"'use strict';

const { readFileSync } = require('fs');

function load(path) {
    const raw = readFileSync(path, 'utf8');
    return JSON.parse(raw);
}
"#;
    let result = entity_merge(base, ours, theirs, "io.js");
    assert!(
        result.is_clean(),
        "Should merge cleanly. Conflicts: {:?}",
        result.conflicts
    );
    let trimmed = result.content.trim_start();
    assert!(
        trimmed.starts_with("'use strict'"),
        "'use strict' must be first line.\nGot:\n{}",
        result.content
    );
    assert!(
        result.content.contains("save"),
        "ours function must be present"
    );
}

/// Scenario 10: both add type imports + modify an interface — complex real-world scenario
#[test]
fn ts_type_imports_plus_interface_modification() {
    let base = r#"import type { BaseConfig } from './base';

export interface AppConfig extends BaseConfig {
  name: string;
  version: string;
}

export function createConfig(name: string): AppConfig {
  return { name, version: '1.0.0' };
}
"#;
    let ours = r#"import type { BaseConfig } from './base';
import type { Logger } from './logger';

export interface AppConfig extends BaseConfig {
  name: string;
  version: string;
  logger?: Logger;
}

export function createConfig(name: string): AppConfig {
  return { name, version: '1.0.0' };
}
"#;
    let theirs = r#"import type { BaseConfig } from './base';
import type { Database } from './db';

export interface AppConfig extends BaseConfig {
  name: string;
  version: string;
  db?: Database;
}

export function createConfig(name: string): AppConfig {
  return { name, version: '1.0.0' };
}
"#;
    let result = entity_merge(base, ours, theirs, "app-config.ts");
    assert!(
        result.content.contains("Logger"),
        "Logger type import or field must be present.\nGot:\n{}",
        result.content
    );
    assert!(
        result.content.contains("Database"),
        "Database type import or field must be present.\nGot:\n{}",
        result.content
    );
    assert!(
        result.content.contains("createConfig"),
        "Untouched function must survive.\nGot:\n{}",
        result.content
    );
}

// =============================================================================
// Batch 2: Comprehensive merge scenarios — real-world edge cases
// =============================================================================

// ---------------------------------------------------------------------------
// 1. Python: both branches add different imports + modify different functions
// ---------------------------------------------------------------------------
#[test]
fn py_both_add_imports_modify_different_functions() {
    let base = r#"import os

def read_file(path):
    with open(path) as f:
        return f.read()

def write_file(path, data):
    with open(path, 'w') as f:
        f.write(data)
"#;
    let ours = r#"import os
import json

def read_file(path):
    with open(path) as f:
        return json.loads(f.read())

def write_file(path, data):
    with open(path, 'w') as f:
        f.write(data)
"#;
    let theirs = r#"import os
import logging

def read_file(path):
    with open(path) as f:
        return f.read()

def write_file(path, data):
    logging.info(f"Writing to {path}")
    with open(path, 'w') as f:
        f.write(data)
"#;
    let result = entity_merge(base, ours, theirs, "fileio.py");
    assert!(
        result.is_clean(),
        "Both add imports + modify different functions should auto-resolve.\nConflicts: {:?}\nContent:\n{}",
        result.conflicts,
        result.content
    );
    assert!(
        result.content.contains("import json"),
        "ours import json must be present"
    );
    assert!(
        result.content.contains("import logging"),
        "theirs import logging must be present"
    );
    assert!(
        result.content.contains("json.loads"),
        "ours read_file modification must be present"
    );
    assert!(
        result.content.contains("logging.info"),
        "theirs write_file modification must be present"
    );
}

// ---------------------------------------------------------------------------
// 2. Rust: both add use + add separate functions
// ---------------------------------------------------------------------------
#[test]
fn rust_both_add_use_and_functions() {
    let base = r#"use std::io;

fn main() {
    println!("hello");
}
"#;
    let ours = r#"use std::io;
use std::fs;

fn main() {
    println!("hello");
}

fn read_config() -> io::Result<String> {
    fs::read_to_string("config.toml")
}
"#;
    let theirs = r#"use std::io;
use std::collections::HashMap;

fn main() {
    println!("hello");
}

fn build_index() -> HashMap<String, usize> {
    HashMap::new()
}
"#;
    let result = entity_merge(base, ours, theirs, "main.rs");
    assert!(
        result.is_clean(),
        "Both add use + new functions should auto-resolve.\nConflicts: {:?}\nContent:\n{}",
        result.conflicts,
        result.content
    );
    assert!(
        result.content.contains("use std::fs"),
        "ours use must be present"
    );
    assert!(
        result.content.contains("use std::collections::HashMap"),
        "theirs use must be present"
    );
    assert!(
        result.content.contains("read_config"),
        "ours function must be present"
    );
    assert!(
        result.content.contains("build_index"),
        "theirs function must be present"
    );
}

// ---------------------------------------------------------------------------
// 3. TS: three functions, ours modifies first + third, theirs modifies second
// ---------------------------------------------------------------------------
#[test]
fn ts_interleaved_modifications_three_functions() {
    let base = r#"export function alpha() {
    return "a";
}

export function beta() {
    return "b";
}

export function gamma() {
    return "g";
}
"#;
    let ours = r#"export function alpha() {
    return "A";
}

export function beta() {
    return "b";
}

export function gamma() {
    return "G";
}
"#;
    let theirs = r#"export function alpha() {
    return "a";
}

export function beta() {
    return "B";
}

export function gamma() {
    return "g";
}
"#;
    let result = entity_merge(base, ours, theirs, "letters.ts");
    assert!(
        result.is_clean(),
        "Interleaved modifications to different functions should auto-resolve.\nConflicts: {:?}\nContent:\n{}",
        result.conflicts,
        result.content
    );
    assert!(
        result.content.contains("\"A\""),
        "alpha must have ours change"
    );
    assert!(
        result.content.contains("\"B\""),
        "beta must have theirs change"
    );
    assert!(
        result.content.contains("\"G\""),
        "gamma must have ours change"
    );
}

// ---------------------------------------------------------------------------
// 4. TS: ours deletes function, theirs adds import used by new function
// ---------------------------------------------------------------------------
#[test]
fn ts_ours_deletes_theirs_adds_import_and_function() {
    let base = r#"import { util } from './util';

export function old() {
    return util();
}

export function keep() {
    return 1;
}
"#;
    let ours = r#"import { util } from './util';

export function keep() {
    return 1;
}
"#;
    let theirs = r#"import { util } from './util';
import { logger } from './logger';

export function old() {
    return util();
}

export function keep() {
    return 1;
}

export function newFunc() {
    logger("created");
    return 2;
}
"#;
    let result = entity_merge(base, ours, theirs, "mixed.ts");
    // old() deleted by ours (unchanged by theirs) => should cleanly delete
    // newFunc added by theirs => should appear
    // logger import added by theirs => should appear
    assert!(
        result.content.contains("newFunc"),
        "theirs new function must be present.\nContent:\n{}",
        result.content
    );
    assert!(
        result.content.contains("keep"),
        "keep must survive.\nContent:\n{}",
        result.content
    );
    assert!(
        result.content.contains("logger"),
        "theirs import must be present.\nContent:\n{}",
        result.content
    );
}

// ---------------------------------------------------------------------------
// 5. TS: both add imports from different modules
// ---------------------------------------------------------------------------
#[test]
fn ts_both_add_imports_from_different_modules() {
    let base = r#"import { useState } from 'react';

export function App() {
    const [count, setCount] = useState(0);
    return count;
}
"#;
    let ours = r#"import { useState } from 'react';
import { Button } from '@mui/material';

export function App() {
    const [count, setCount] = useState(0);
    return count;
}
"#;
    let theirs = r#"import { useState } from 'react';
import { format } from 'date-fns';

export function App() {
    const [count, setCount] = useState(0);
    return count;
}
"#;
    let result = entity_merge(base, ours, theirs, "App.tsx");
    assert!(
        result.content.contains("Button"),
        "ours MUI import must be present.\nContent:\n{}",
        result.content
    );
    assert!(
        result.content.contains("date-fns"),
        "theirs date-fns import must be present.\nContent:\n{}",
        result.content
    );
    assert!(
        result.content.contains("useState"),
        "original useState import must survive.\nContent:\n{}",
        result.content
    );
    assert!(
        result.content.contains("App"),
        "App function must survive.\nContent:\n{}",
        result.content
    );
}

// ---------------------------------------------------------------------------
// 6. JSON: both add keys in different sections
// ---------------------------------------------------------------------------
#[test]
fn json_both_add_keys_in_different_sections() {
    let base = r#"{
  "name": "my-app",
  "version": "1.0.0",
  "scripts": {
    "build": "tsc"
  }
}
"#;
    let ours = r#"{
  "name": "my-app",
  "version": "1.0.0",
  "scripts": {
    "build": "tsc",
    "test": "jest"
  }
}
"#;
    let theirs = r#"{
  "name": "my-app",
  "version": "1.1.0",
  "scripts": {
    "build": "tsc"
  }
}
"#;
    let result = entity_merge(base, ours, theirs, "package.json");
    assert!(
        result.content.contains("test"),
        "ours test script must be present.\nContent:\n{}",
        result.content
    );
    assert!(
        result.content.contains("1.1.0"),
        "theirs version bump must be present.\nContent:\n{}",
        result.content
    );
}

// ---------------------------------------------------------------------------
// 7. TS: both modify the same import line differently (conflict expected)
// ---------------------------------------------------------------------------
#[test]
fn ts_both_modify_same_import_conflict() {
    let base = r#"import { foo } from './utils';

export function run() {
    return foo();
}
"#;
    let ours = r#"import { foo, bar } from './utils';

export function run() {
    return foo();
}
"#;
    let theirs = r#"import { foo, baz } from './utils';

export function run() {
    return foo();
}
"#;
    let result = entity_merge(base, ours, theirs, "run.ts");
    // Both modify the same import line — the commutative import merger should
    // handle this (both add to same import source). The key check is that
    // neither bar nor baz is silently dropped.
    let has_bar = result.content.contains("bar");
    let has_baz = result.content.contains("baz");
    assert!(
        has_bar && has_baz,
        "Both bar and baz must be present (merged or conflicted).\nContent:\n{}",
        result.content
    );
}

// ---------------------------------------------------------------------------
// 8. Go: both add different functions
// ---------------------------------------------------------------------------
#[test]
fn go_both_add_imports_and_functions() {
    let base = r#"package main

import "fmt"

func main() {
	fmt.Println("hello")
}
"#;
    let ours = r#"package main

import "fmt"

func main() {
	fmt.Println("hello")
}

func helper() string {
	return "help"
}
"#;
    let theirs = r#"package main

import "fmt"

func main() {
	fmt.Println("hello world")
}
"#;
    let result = entity_merge(base, ours, theirs, "main.go");
    assert!(
        result.is_clean(),
        "Ours adds function, theirs modifies main — should auto-resolve.\nConflicts: {:?}\nContent:\n{}",
        result.conflicts,
        result.content
    );
    assert!(
        result.content.contains("helper"),
        "ours helper must be present"
    );
    assert!(
        result.content.contains("hello world"),
        "theirs main modification must be present"
    );
}

// ---------------------------------------------------------------------------
// 9. TS: one branch adds JSDoc comment to a function
// ---------------------------------------------------------------------------
#[test]
fn ts_one_adds_jsdoc_other_adds_function() {
    let base = r#"export function calculate(x: number): number {
    return x * 2;
}
"#;
    let ours = r#"/**
 * Doubles the input value.
 * @param x - The input number
 * @returns The doubled value
 */
export function calculate(x: number): number {
    return x * 2;
}
"#;
    let theirs = r#"export function calculate(x: number): number {
    return x * 2;
}

export function triple(x: number): number {
    return x * 3;
}
"#;
    let result = entity_merge(base, ours, theirs, "math.ts");
    assert!(
        result.content.contains("triple"),
        "theirs new function must be present.\nContent:\n{}",
        result.content
    );
    assert!(
        result.content.contains("Doubles the input"),
        "ours JSDoc must be present.\nContent:\n{}",
        result.content
    );
}

// ---------------------------------------------------------------------------
// 10. TS: both branches add functions at the end of a large file
// ---------------------------------------------------------------------------
#[test]
fn ts_both_add_at_end_of_large_file() {
    let base = r#"export function one() { return 1; }

export function two() { return 2; }

export function three() { return 3; }

export function four() { return 4; }

export function five() { return 5; }
"#;
    let ours = r#"export function one() { return 1; }

export function two() { return 2; }

export function three() { return 3; }

export function four() { return 4; }

export function five() { return 5; }

export function six() { return 6; }
"#;
    let theirs = r#"export function one() { return 1; }

export function two() { return 2; }

export function three() { return 3; }

export function four() { return 4; }

export function five() { return 5; }

export function seven() { return 7; }
"#;
    let result = entity_merge(base, ours, theirs, "numbers.ts");
    assert!(
        result.is_clean(),
        "Both adding at end should auto-resolve.\nConflicts: {:?}\nContent:\n{}",
        result.conflicts,
        result.content
    );
    assert!(result.content.contains("six"), "ours six must be present");
    assert!(
        result.content.contains("seven"),
        "theirs seven must be present"
    );
    // All original functions must survive
    for name in &["one", "two", "three", "four", "five"] {
        assert!(
            result.content.contains(name),
            "{} must survive.\nContent:\n{}",
            name,
            result.content
        );
    }
}

// ---------------------------------------------------------------------------
// 11. Rust: one branch adds impl method, other adds trait impl
// ---------------------------------------------------------------------------
#[test]
fn rust_one_adds_method_other_modifies_existing() {
    let base = r#"struct Counter {
    value: u32,
}

impl Counter {
    fn new() -> Self {
        Counter { value: 0 }
    }

    fn increment(&mut self) {
        self.value += 1;
    }
}
"#;
    let ours = r#"struct Counter {
    value: u32,
}

impl Counter {
    fn new() -> Self {
        Counter { value: 0 }
    }

    fn increment(&mut self) {
        self.value += 1;
    }

    fn reset(&mut self) {
        self.value = 0;
    }
}
"#;
    let theirs = r#"struct Counter {
    value: u32,
}

impl Counter {
    fn new() -> Self {
        Counter { value: 0 }
    }

    fn increment(&mut self) {
        self.value += 2;
    }
}
"#;
    let result = entity_merge(base, ours, theirs, "counter.rs");
    assert!(
        result.is_clean(),
        "Adding method + modifying different method should auto-resolve.\nConflicts: {:?}\nContent:\n{}",
        result.conflicts,
        result.content
    );
    assert!(
        result.content.contains("reset"),
        "ours reset method must be present"
    );
    assert!(
        result.content.contains("self.value += 2"),
        "theirs increment change must be present"
    );
}

// ---------------------------------------------------------------------------
// 12. Python: one adds decorator, other modifies function body
// ---------------------------------------------------------------------------
#[test]
fn py_one_adds_decorator_other_modifies_body() {
    let base = r#"def process(data):
    return data.strip()
"#;
    let ours = r#"def process(data):
    return data.strip().lower()
"#;
    let theirs = r#"def process(data):
    return data.strip()

def validate(data):
    return len(data) > 0
"#;
    let result = entity_merge(base, ours, theirs, "process.py");
    assert!(
        result.is_clean(),
        "One modifies, other adds new function should auto-resolve.\nConflicts: {:?}\nContent:\n{}",
        result.conflicts,
        result.content
    );
    assert!(
        result.content.contains(".lower()"),
        "ours body change must be present"
    );
    assert!(
        result.content.contains("validate"),
        "theirs new function must be present"
    );
}

// ---------------------------------------------------------------------------
// 13. TS: import removal by one side, import addition by other
// ---------------------------------------------------------------------------
#[test]
fn ts_one_removes_import_other_adds_import() {
    let base = r#"import { unused } from './unused';
import { used } from './used';

export function run() {
    return used();
}
"#;
    let ours = r#"import { used } from './used';

export function run() {
    return used();
}
"#;
    let theirs = r#"import { unused } from './unused';
import { used } from './used';
import { extra } from './extra';

export function run() {
    return used();
}
"#;
    let result = entity_merge(base, ours, theirs, "imports.ts");
    assert!(
        result.content.contains("extra"),
        "theirs added import must be present.\nContent:\n{}",
        result.content
    );
    assert!(
        result.content.contains("used"),
        "used import must survive.\nContent:\n{}",
        result.content
    );
}

// ---------------------------------------------------------------------------
// 14. TS: both branches modify interface — same field differently (conflict)
// ---------------------------------------------------------------------------
#[test]
fn ts_interface_same_field_both_modify_conflict() {
    let base = r#"export interface Config {
    timeout: number;
    retries: number;
}

export function getConfig(): Config {
    return { timeout: 1000, retries: 3 };
}
"#;
    let ours = r#"export interface Config {
    timeout: number;
    retries: number;
    debug: boolean;
}

export function getConfig(): Config {
    return { timeout: 1000, retries: 3, debug: false };
}
"#;
    let theirs = r#"export interface Config {
    timeout: number;
    retries: number;
    verbose: boolean;
}

export function getConfig(): Config {
    return { timeout: 1000, retries: 3, verbose: true };
}
"#;
    let result = entity_merge(base, ours, theirs, "config.ts");
    // Both modify Config (add different fields) and getConfig (add different defaults)
    // These are modifications to the same entities — could be conflict or auto-resolve
    // depending on whether weave handles line-level within entities
    let has_debug = result.content.contains("debug");
    let has_verbose = result.content.contains("verbose");
    assert!(
        has_debug && has_verbose,
        "Both debug and verbose must be present (merged or in conflict markers).\nContent:\n{}",
        result.content
    );
}

// ---------------------------------------------------------------------------
// 15. TS: re-export barrel file — both add different re-exports
// ---------------------------------------------------------------------------
#[test]
fn ts_barrel_both_add_reexports() {
    let base = r#"export { foo } from './foo';
export { bar } from './bar';
"#;
    let ours = r#"export { foo } from './foo';
export { bar } from './bar';
export { baz } from './baz';
"#;
    let theirs = r#"export { foo } from './foo';
export { bar } from './bar';
export { qux } from './qux';
"#;
    let result = entity_merge(base, ours, theirs, "index.ts");
    assert!(
        result.content.contains("baz"),
        "ours baz re-export must be present.\nContent:\n{}",
        result.content
    );
    assert!(
        result.content.contains("qux"),
        "theirs qux re-export must be present.\nContent:\n{}",
        result.content
    );
}

// ---------------------------------------------------------------------------
// 16. TS: empty base — ours creates full file, theirs creates different file
// ---------------------------------------------------------------------------
#[test]
fn ts_empty_base_both_create_different_files() {
    let base = "";
    let ours = r#"export function ours() {
    return "ours";
}
"#;
    let theirs = r#"export function theirs() {
    return "theirs";
}
"#;
    let result = entity_merge(base, ours, theirs, "new.ts");
    // Both create entirely different content from empty — should include both
    assert!(
        result.content.contains("ours"),
        "ours function must be present.\nContent:\n{}",
        result.content
    );
    assert!(
        result.content.contains("theirs"),
        "theirs function must be present.\nContent:\n{}",
        result.content
    );
}

// ---------------------------------------------------------------------------
// 17. Python: from-imports — both add different symbols from same module
// ---------------------------------------------------------------------------
#[test]
fn py_both_add_from_imports_same_module() {
    let base = r#"from os.path import join

def build_path(base, name):
    return join(base, name)
"#;
    let ours = r#"from os.path import join, exists

def build_path(base, name):
    return join(base, name)
"#;
    let theirs = r#"from os.path import join, dirname

def build_path(base, name):
    return join(base, name)
"#;
    let result = entity_merge(base, ours, theirs, "paths.py");
    assert!(
        result.content.contains("exists"),
        "ours exists import must be present.\nContent:\n{}",
        result.content
    );
    assert!(
        result.content.contains("dirname"),
        "theirs dirname import must be present.\nContent:\n{}",
        result.content
    );
}

// ---------------------------------------------------------------------------
// 18. TS: large class — ours adds method at top, theirs adds at bottom
// ---------------------------------------------------------------------------
#[test]
fn ts_class_add_methods_at_different_positions() {
    let base = r#"export class Service {
    constructor(private name: string) {}

    getName(): string {
        return this.name;
    }

    isActive(): boolean {
        return true;
    }
}
"#;
    let ours = r#"export class Service {
    constructor(private name: string) {}

    getId(): string {
        return this.name.toLowerCase();
    }

    getName(): string {
        return this.name;
    }

    isActive(): boolean {
        return true;
    }
}
"#;
    let theirs = r#"export class Service {
    constructor(private name: string) {}

    getName(): string {
        return this.name;
    }

    isActive(): boolean {
        return true;
    }

    shutdown(): void {
        console.log("shutting down");
    }
}
"#;
    let result = entity_merge(base, ours, theirs, "service.ts");
    assert!(
        result.is_clean(),
        "Adding methods at different positions should auto-resolve.\nConflicts: {:?}\nContent:\n{}",
        result.conflicts,
        result.content
    );
    assert!(
        result.content.contains("getId"),
        "ours getId must be present"
    );
    assert!(
        result.content.contains("shutdown"),
        "theirs shutdown must be present"
    );
    assert!(
        result.content.contains("getName"),
        "original getName must survive"
    );
    assert!(
        result.content.contains("isActive"),
        "original isActive must survive"
    );
}

// ---------------------------------------------------------------------------
// 19. TS: both branches change the default export
// ---------------------------------------------------------------------------
#[test]
fn ts_both_modify_default_export() {
    let base = r#"const config = {
    port: 3000,
    host: "localhost",
};

export default config;
"#;
    let ours = r#"const config = {
    port: 8080,
    host: "localhost",
};

export default config;
"#;
    let theirs = r#"const config = {
    port: 3000,
    host: "0.0.0.0",
};

export default config;
"#;
    let result = entity_merge(base, ours, theirs, "config.ts");
    // Both modify the same entity (config) but different fields — may auto-resolve
    // via line-level diffing or may conflict
    let has_8080 = result.content.contains("8080");
    let has_0000 = result.content.contains("0.0.0.0");
    assert!(
        has_8080 && has_0000,
        "Both changes must be present (merged or in conflict markers).\nContent:\n{}",
        result.content
    );
}

// ---------------------------------------------------------------------------
// 20. C: both add different functions + modify different existing ones
// ---------------------------------------------------------------------------
#[test]
fn c_both_modify_and_add_different() {
    let base = r#"#include <stdio.h>

void greet() {
    printf("hello\n");
}

int add(int a, int b) {
    return a + b;
}
"#;
    let ours = r#"#include <stdio.h>

void greet() {
    printf("hello world\n");
}

int add(int a, int b) {
    return a + b;
}

int multiply(int a, int b) {
    return a * b;
}
"#;
    let theirs = r#"#include <stdio.h>

void greet() {
    printf("hello\n");
}

int add(int a, int b) {
    return a + b + 0;
}

int subtract(int a, int b) {
    return a - b;
}
"#;
    let result = entity_merge(base, ours, theirs, "math.c");
    assert!(
        result.is_clean(),
        "Modifying + adding different functions should auto-resolve.\nConflicts: {:?}\nContent:\n{}",
        result.conflicts,
        result.content
    );
    assert!(
        result.content.contains("multiply"),
        "ours multiply must be present"
    );
    assert!(
        result.content.contains("subtract"),
        "theirs subtract must be present"
    );
    assert!(
        result.content.contains("hello world"),
        "ours greet modification must be present"
    );
    assert!(
        result.content.contains("a + b + 0"),
        "theirs add modification must be present"
    );
}

/// Check if a needle appears only inside conflict marker blocks
fn is_inside_conflict_markers(content: &str, needle: &str) -> bool {
    let mut in_conflict = false;
    for line in content.lines() {
        if line.starts_with("<<<<<<<") {
            in_conflict = true;
        } else if line.starts_with(">>>>>>>") {
            in_conflict = false;
        } else if in_conflict && line.contains(needle) {
            return true;
        }
    }
    false
}
