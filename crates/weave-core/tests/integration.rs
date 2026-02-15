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
    assert!(result.content.contains("cache.get"), "Should contain ours's caching change");
    assert!(result.content.contains("email required"), "Should contain theirs's validation change");
    assert!(result.content.contains("deleteUser"), "Should preserve unchanged method");
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
    assert!(result.content.contains("console.log"), "Should contain modified add");
    assert!(result.content.contains("multiply"), "Should contain new method");
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
    assert!(result.content.contains("sayHello"), "Should have renamed function");
    // Should have the modified farewell
    assert!(result.content.contains("See you later"), "Should have modified farewell");
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
    assert!(result.content.contains("raise ValueError"), "Should contain ours's validation");
    assert!(result.content.contains("self.cache.get"), "Should contain theirs's caching");
    assert!(result.content.contains("def update"), "Should preserve unchanged methods");
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
    assert!(result.content.contains("toUpperCase"), "Should take the real change (theirs)");
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
    assert!(result.content.contains("logger.info"), "Should contain ours change");
    assert!(result.content.contains("validateUser"), "Should contain theirs change");
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
    assert!(result.content.contains("@Cacheable"), "Should contain ours annotation");
    assert!(result.content.contains("@RateLimit"), "Should contain theirs annotation");
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
    assert!(result.content.contains("log_debug"), "Should contain ours change");
    assert!(result.content.contains("NULL"), "Should contain theirs change");
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
    assert!(result.content.contains("console.log(\"fetching\""), "Should have theirs modification");
    assert!(result.content.contains("deleteUser"), "Should have all methods");
    assert!(result.content.contains("createUser"), "Should have all methods");
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
    assert!(result.content.contains("multiply"), "Should have ours method");
    assert!(result.content.contains("divide"), "Should have theirs method");
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
    assert!(result.content.contains("multiply"), "Should have ours method");
    assert!(result.content.contains("divide"), "Should have theirs method");
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
    assert!(result.content.contains("HandlePost"), "Should have ours function");
    assert!(result.content.contains("HandleDelete"), "Should have theirs function");
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
    assert!(result.content.contains("\"disabled\""), "Should have modified variant");
    assert!(result.content.contains("Deleted"), "Should have new variant");
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
    assert!(!result.is_clean(), "Should have a conflict on the add method");

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
    assert!(result.content.contains("public int subtract"), "subtract should be in output");
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
    eprintln!("clean: {}, conflicts: {}", result.is_clean(), result.conflicts.len());

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
            "{} should NOT be inside conflict markers", method
        );
        assert!(result.content.contains(method), "{} should be in output", method);
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
            "{} should NOT be inside conflict markers", method
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
            "{} should NOT be inside conflict markers", method
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
            "{} should NOT be inside conflict markers", method
        );
    }
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
