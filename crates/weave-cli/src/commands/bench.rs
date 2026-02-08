use std::time::Instant;

use weave_core::entity_merge;

/// Run merge benchmarks comparing weave's entity-level merge against
/// git's line-level merge (simulated via diffy).
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("weave merge benchmark");
    println!("=====================\n");

    let scenarios = vec![
        Scenario {
            name: "Different functions modified",
            description: "Two agents modify different functions in the same file",
            file_path: "app.ts",
            base: r#"import { config } from './config';

export function processData(input: string): string {
    return input.trim();
}

export function validateInput(input: string): boolean {
    return input.length > 0;
}

export function formatOutput(data: string): string {
    return `Result: ${data}`;
}
"#,
            ours: r#"import { config } from './config';

export function processData(input: string): string {
    const cleaned = input.trim();
    console.log("Processing:", cleaned);
    return cleaned.toUpperCase();
}

export function validateInput(input: string): boolean {
    return input.length > 0;
}

export function formatOutput(data: string): string {
    return `Result: ${data}`;
}
"#,
            theirs: r#"import { config } from './config';

export function processData(input: string): string {
    return input.trim();
}

export function validateInput(input: string): boolean {
    if (!input) return false;
    return input.length > 0 && input.length < 1000;
}

export function formatOutput(data: string): string {
    return `Result: ${data}`;
}
"#,
        },
        Scenario {
            name: "Different class methods modified",
            description: "Two agents modify different methods in the same class",
            file_path: "service.ts",
            base: r#"export class UserService {
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
"#,
            ours: r#"export class UserService {
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
"#,
            theirs: r#"export class UserService {
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
"#,
        },
        Scenario {
            name: "Both add different imports",
            description: "Two agents add different imports — commutative merge",
            file_path: "imports.ts",
            base: r#"import { foo } from './foo';
import { bar } from './bar';

export function main() {
    return foo() + bar();
}
"#,
            ours: r#"import { foo } from './foo';
import { bar } from './bar';
import { baz } from './baz';

export function main() {
    return foo() + bar();
}
"#,
            theirs: r#"import { foo } from './foo';
import { bar } from './bar';
import { qux } from './qux';

export function main() {
    return foo() + bar();
}
"#,
        },
        Scenario {
            name: "Class: getUser + createUser (4 methods)",
            description: "Bigger class — both agents edit different methods among 4",
            file_path: "big-service.ts",
            base: r#"export class DataService {
    fetch(id: string): Data {
        return this.api.get(id);
    }

    transform(data: Data): Output {
        return { value: data.raw };
    }

    validate(input: Input): boolean {
        return input.value != null;
    }

    save(output: Output): void {
        this.db.insert(output);
    }
}
"#,
            ours: r#"export class DataService {
    fetch(id: string): Data {
        const cached = this.cache.get(id);
        if (cached) return cached;
        const result = this.api.get(id);
        this.cache.set(id, result);
        return result;
    }

    transform(data: Data): Output {
        return { value: data.raw };
    }

    validate(input: Input): boolean {
        return input.value != null;
    }

    save(output: Output): void {
        this.db.insert(output);
    }
}
"#,
            theirs: r#"export class DataService {
    fetch(id: string): Data {
        return this.api.get(id);
    }

    transform(data: Data): Output {
        const cleaned = this.sanitize(data.raw);
        return { value: cleaned, timestamp: Date.now() };
    }

    validate(input: Input): boolean {
        return input.value != null;
    }

    save(output: Output): void {
        this.db.insert(output);
    }
}
"#,
        },
        Scenario {
            name: "One adds function, other modifies existing",
            description: "Agent A adds a new function, Agent B modifies an existing one",
            file_path: "utils.ts",
            base: r#"export function helper() {
    return "help";
}
"#,
            ours: r#"export function helper() {
    return "help";
}

export function newFeature() {
    return "new feature by agent A";
}
"#,
            theirs: r#"export function helper() {
    console.log("helper called");
    return "improved help";
}
"#,
        },
        Scenario {
            name: "Adjacent function changes (stress test)",
            description: "Two agents modify adjacent functions — tests merge precision",
            file_path: "adjacent.ts",
            base: r#"export function alpha() {
    return "a";
}
export function beta() {
    return "b";
}
"#,
            ours: r#"export function alpha() {
    return "A";
}
export function beta() {
    return "b";
}
"#,
            theirs: r#"export function alpha() {
    return "a";
}
export function beta() {
    return "B";
}
"#,
        },
        Scenario {
            name: "Python: different methods in same class",
            description: "Two agents modify different methods in a Python class",
            file_path: "service.py",
            base: r#"class DataProcessor:
    def load(self, path):
        with open(path) as f:
            return f.read()

    def transform(self, data):
        return data.strip()

    def save(self, data, path):
        with open(path, 'w') as f:
            f.write(data)
"#,
            ours: r#"class DataProcessor:
    def load(self, path):
        import json
        with open(path) as f:
            return json.load(f)

    def transform(self, data):
        return data.strip()

    def save(self, data, path):
        with open(path, 'w') as f:
            f.write(data)
"#,
            theirs: r#"class DataProcessor:
    def load(self, path):
        with open(path) as f:
            return f.read()

    def transform(self, data):
        cleaned = data.strip()
        return cleaned.lower()

    def save(self, data, path):
        with open(path, 'w') as f:
            f.write(data)
"#,
        },
        Scenario {
            name: "Python: adjacent methods (harder)",
            description: "Two agents modify adjacent methods in Python class — diffy often fails",
            file_path: "service.py",
            base: r#"class Service:
    def create(self, data):
        return self.db.insert(data)

    def read(self, id):
        return self.db.find(id)

    def update(self, id, data):
        self.db.update(id, data)

    def delete(self, id):
        self.db.remove(id)
"#,
            ours: r#"class Service:
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
"#,
            theirs: r#"class Service:
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
"#,
        },
        Scenario {
            name: "TS: both add exports at end",
            description: "Both agents add different named exports — a very common pattern",
            file_path: "exports.ts",
            base: r#"export function alpha(): string {
    return "alpha";
}

export function beta(): string {
    return "beta";
}
"#,
            ours: r#"export function alpha(): string {
    return "alpha";
}

export function beta(): string {
    return "beta";
}

export function gamma(): string {
    return "gamma - from agent A";
}
"#,
            theirs: r#"export function alpha(): string {
    return "alpha";
}

export function beta(): string {
    return "beta";
}

export function delta(): string {
    return "delta - from agent B";
}
"#,
        },
        Scenario {
            name: "Reformat vs modify (whitespace-aware)",
            description: "One agent reformats, other makes real change — whitespace detection",
            file_path: "format.ts",
            base: r#"export function process(data: string): string {
    return data.trim();
}

export function validate(input: string): boolean {
    return input.length > 0;
}
"#,
            ours: r#"export function process(data: string): string {
      return data.trim();
}

export function validate(input: string): boolean {
      return input.length > 0;
}
"#,
            theirs: r#"export function process(data: string): string {
    const cleaned = data.trim();
    return cleaned.toUpperCase();
}

export function validate(input: string): boolean {
    return input.length > 0;
}
"#,
        },
        // --- NEW: scenarios targeting known git false-conflict patterns ---
        Scenario {
            name: "Both add new functions at end of file",
            description: "Both agents append different functions — git conflicts on insertion point",
            file_path: "append.ts",
            base: r#"export function existing() {
    return "exists";
}
"#,
            ours: r#"export function existing() {
    return "exists";
}

export function featureA() {
    return "added by agent A";
}
"#,
            theirs: r#"export function existing() {
    return "exists";
}

export function featureB() {
    return "added by agent B";
}
"#,
        },
        Scenario {
            name: "Both add methods to class at end",
            description: "Both agents add different methods to end of class — git conflicts",
            file_path: "class-append.ts",
            base: r#"export class Router {
    get(path: string) {
        return this.routes.get(path);
    }
}
"#,
            ours: r#"export class Router {
    get(path: string) {
        return this.routes.get(path);
    }

    post(path: string, handler: Handler) {
        this.routes.set(path, handler);
    }
}
"#,
            theirs: r#"export class Router {
    get(path: string) {
        return this.routes.get(path);
    }

    delete(path: string) {
        this.routes.delete(path);
    }
}
"#,
        },
        Scenario {
            name: "Rust: both add different use statements",
            description: "Both agents add different use imports — commutative merge",
            file_path: "lib.rs",
            base: r#"use std::io;
use std::fs;

pub fn process() {
    println!("processing");
}
"#,
            ours: r#"use std::io;
use std::fs;
use std::path::PathBuf;

pub fn process() {
    println!("processing");
}
"#,
            theirs: r#"use std::io;
use std::fs;
use std::collections::HashMap;

pub fn process() {
    println!("processing");
}
"#,
        },
        Scenario {
            name: "Python: both add different imports",
            description: "Both agents add different Python imports — commutative merge",
            file_path: "app.py",
            base: r#"import os
import sys

def main():
    print("hello")
"#,
            ours: r#"import os
import sys
import json

def main():
    print("hello")
"#,
            theirs: r#"import os
import sys
import pathlib

def main():
    print("hello")
"#,
        },
        Scenario {
            name: "Class: modify method + add new method",
            description: "Agent A modifies a method, Agent B adds a new one — tests mixed changes",
            file_path: "mixed.ts",
            base: r#"export class Cache {
    get(key: string): string | null {
        return this.store[key] || null;
    }

    set(key: string, value: string): void {
        this.store[key] = value;
    }
}
"#,
            ours: r#"export class Cache {
    get(key: string): string | null {
        const val = this.store[key];
        if (!val) return null;
        this.hits++;
        return val;
    }

    set(key: string, value: string): void {
        this.store[key] = value;
    }
}
"#,
            theirs: r#"export class Cache {
    get(key: string): string | null {
        return this.store[key] || null;
    }

    set(key: string, value: string): void {
        this.store[key] = value;
    }

    delete(key: string): boolean {
        if (this.store[key]) {
            delete this.store[key];
            return true;
        }
        return false;
    }
}
"#,
        },
        Scenario {
            name: "Both add functions between existing ones",
            description: "Both insert different functions in the middle — git conflicts on position",
            file_path: "insert-middle.ts",
            base: r#"export function first() {
    return 1;
}

export function last() {
    return 99;
}
"#,
            ours: r#"export function first() {
    return 1;
}

export function middleA() {
    return "from agent A";
}

export function last() {
    return 99;
}
"#,
            theirs: r#"export function first() {
    return 1;
}

export function middleB() {
    return "from agent B";
}

export function last() {
    return 99;
}
"#,
        },
        // --- New scenarios: decorator/annotation merge ---
        Scenario {
            name: "Python: both add different decorators",
            description: "Both add different decorators to same function — git conflicts",
            file_path: "decorators.py",
            base: r#"def process(data):
    validated = validate(data)
    return transform(validated)

def helper():
    return True
"#,
            ours: r#"@cache
@log_calls
def process(data):
    validated = validate(data)
    return transform(validated)

def helper():
    return True
"#,
            theirs: r#"@deprecated("use process_v2")
@retry(max_attempts=3)
def process(data):
    validated = validate(data)
    return transform(validated)

def helper():
    return True
"#,
        },
        Scenario {
            name: "Decorator + body change",
            description: "One adds decorator, other modifies body — should merge",
            file_path: "deco-body.py",
            base: r#"def fetch(url):
    return requests.get(url)

def parse(data):
    return json.loads(data)
"#,
            ours: r#"@cache(ttl=300)
def fetch(url):
    return requests.get(url)

def parse(data):
    return json.loads(data)
"#,
            theirs: r#"def fetch(url):
    response = requests.get(url, timeout=30)
    response.raise_for_status()
    return response

def parse(data):
    return json.loads(data)
"#,
        },
        Scenario {
            name: "TS: class method decorators",
            description: "Both add different decorators to class method",
            file_path: "class-deco.ts",
            base: r#"class UserService {
    getUser(id: string) {
        return this.db.find(id);
    }

    createUser(data: any) {
        return this.db.create(data);
    }
}
"#,
            ours: r#"class UserService {
    @Cacheable({ ttl: 60 })
    getUser(id: string) {
        return this.db.find(id);
    }

    createUser(data: any) {
        return this.db.create(data);
    }
}
"#,
            theirs: r#"class UserService {
    @RateLimit(100)
    getUser(id: string) {
        return this.db.find(id);
    }

    createUser(data: any) {
        return this.db.create(data);
    }
}
"#,
        },
        // --- New scenarios: struct/interface/enum field merge ---
        Scenario {
            name: "TS: interface field additions",
            description: "Both add different fields to same interface",
            file_path: "types.ts",
            base: r#"interface UserConfig {
    name: string;
    email: string;
}

export function getUser(): UserConfig {
    return { name: "", email: "" };
}
"#,
            ours: r#"interface UserConfig {
    name: string;
    email: string;
    age: number;
    phone: string;
}

export function getUser(): UserConfig {
    return { name: "", email: "" };
}
"#,
            theirs: r#"interface UserConfig {
    name: string;
    email: string;
    role: string;
    isActive: boolean;
}

export function getUser(): UserConfig {
    return { name: "", email: "" };
}
"#,
        },
        Scenario {
            name: "Rust: enum variant additions",
            description: "Both add different variants to same enum",
            file_path: "types.rs",
            base: r#"enum Status {
    Active,
    Inactive,
}

fn check_status(s: &Status) -> bool {
    matches!(s, Status::Active)
}
"#,
            ours: r#"enum Status {
    Active,
    Inactive,
    Pending,
    Suspended,
}

fn check_status(s: &Status) -> bool {
    matches!(s, Status::Active)
}
"#,
            theirs: r#"enum Status {
    Active,
    Inactive,
    Archived,
    Deleted,
}

fn check_status(s: &Status) -> bool {
    matches!(s, Status::Active)
}
"#,
        },
        // --- New scenarios: Java and C merge ---
        Scenario {
            name: "Java: different methods in same class",
            description: "Both modify different methods in a Java class",
            file_path: "UserService.java",
            base: r#"public class UserService {
    public User getUser(String id) {
        return db.find(id);
    }

    public void createUser(User user) {
        db.save(user);
    }

    public void deleteUser(String id) {
        db.delete(id);
    }
}
"#,
            ours: r#"public class UserService {
    public User getUser(String id) {
        User user = db.find(id);
        logger.info("Found user: " + id);
        return user;
    }

    public void createUser(User user) {
        db.save(user);
    }

    public void deleteUser(String id) {
        db.delete(id);
    }
}
"#,
            theirs: r#"public class UserService {
    public User getUser(String id) {
        return db.find(id);
    }

    public void createUser(User user) {
        validateUser(user);
        db.save(user);
        eventBus.publish(new UserCreatedEvent(user));
    }

    public void deleteUser(String id) {
        db.delete(id);
    }
}
"#,
        },
        Scenario {
            name: "Java: both add annotations",
            description: "Both add different annotations to same method",
            file_path: "Controller.java",
            base: r#"public class Controller {
    public Response handle(Request req) {
        return service.process(req);
    }

    public Response health() {
        return Response.ok();
    }
}
"#,
            ours: r#"public class Controller {
    @Cacheable(ttl = 60)
    public Response handle(Request req) {
        return service.process(req);
    }

    public Response health() {
        return Response.ok();
    }
}
"#,
            theirs: r#"public class Controller {
    @RateLimit(100)
    public Response handle(Request req) {
        return service.process(req);
    }

    public Response health() {
        return Response.ok();
    }
}
"#,
        },
        Scenario {
            name: "C: different functions modified",
            description: "Both modify different functions in a C file",
            file_path: "utils.c",
            base: r#"void init(Config* cfg) {
    cfg->ready = 1;
}

int process(Data* data) {
    return data->value * 2;
}

void cleanup(Config* cfg) {
    cfg->ready = 0;
}
"#,
            ours: r#"void init(Config* cfg) {
    cfg->ready = 1;
    log_debug("initialized");
}

int process(Data* data) {
    return data->value * 2;
}

void cleanup(Config* cfg) {
    cfg->ready = 0;
}
"#,
            theirs: r#"void init(Config* cfg) {
    cfg->ready = 1;
}

int process(Data* data) {
    if (data == NULL) return -1;
    return data->value * 2;
}

void cleanup(Config* cfg) {
    cfg->ready = 0;
}
"#,
        },
        // === Method reordering ===
        Scenario {
            name: "TS: method reorder + modification",
            description: "One reorders methods in class, other modifies a method",
            file_path: "service.ts",
            base: r#"class Service {
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
"#,
            ours: r#"class Service {
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
"#,
            theirs: r#"class Service {
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
"#,
        },
        // === Python class methods ===
        Scenario {
            name: "Python: both add class methods",
            description: "Both add different methods to same Python class",
            file_path: "calculator.py",
            base: "class Calculator:\n    def add(self, a, b):\n        return a + b\n",
            ours: "class Calculator:\n    def add(self, a, b):\n        return a + b\n\n    def multiply(self, a, b):\n        return a * b\n",
            theirs: "class Calculator:\n    def add(self, a, b):\n        return a + b\n\n    def divide(self, a, b):\n        return a / b\n",
        },
        // === Rust impl methods ===
        Scenario {
            name: "Rust: both add impl methods",
            description: "Both add different methods to same Rust impl block",
            file_path: "calc.rs",
            base: r#"impl Calculator {
    fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}
"#,
            ours: r#"impl Calculator {
    fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }
}
"#,
            theirs: r#"impl Calculator {
    fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    fn divide(&self, a: i32, b: i32) -> i32 {
        a / b
    }
}
"#,
        },
        // === Enum modify + add ===
        Scenario {
            name: "TS: enum modify variant + add variant",
            description: "One modifies existing variant value, other adds new variant",
            file_path: "status.ts",
            base: "enum Status {\n    Active = \"active\",\n    Inactive = \"inactive\",\n    Pending = \"pending\",\n}\n",
            ours: "enum Status {\n    Active = \"active\",\n    Inactive = \"disabled\",\n    Pending = \"pending\",\n}\n",
            theirs: "enum Status {\n    Active = \"active\",\n    Inactive = \"inactive\",\n    Pending = \"pending\",\n    Deleted = \"deleted\",\n}\n",
        },
        // === Doc comment + body ===
        Scenario {
            name: "TS: add JSDoc + modify function body",
            description: "One adds JSDoc comment, other modifies function body",
            file_path: "math.ts",
            base: r#"export function calculate(a: number, b: number): number {
    return a + b;
}
"#,
            ours: r#"/**
 * Calculate the sum of two numbers.
 * @param a - First number
 * @param b - Second number
 */
export function calculate(a: number, b: number): number {
    return a + b;
}
"#,
            theirs: r#"export function calculate(a: number, b: number): number {
    const result = a + b;
    console.log("result:", result);
    return result;
}
"#,
        },
        // === Doc comment bundling ===
        Scenario {
            name: "Rust: both add doc comments to different fns",
            description: "Both add doc comments to different functions via comment bundling",
            file_path: "math.rs",
            base: "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\nfn subtract(a: i32, b: i32) -> i32 {\n    a - b\n}\n",
            ours: "/// Adds two numbers.\nfn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\nfn subtract(a: i32, b: i32) -> i32 {\n    a - b\n}\n",
            theirs: "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\n/// Subtracts b from a.\nfn subtract(a: i32, b: i32) -> i32 {\n    a - b\n}\n",
        },
        // === Go: both add different functions ===
        Scenario {
            name: "Go: both add different functions",
            description: "Both add different functions to same Go file",
            file_path: "handlers.go",
            base: r#"package handlers

func HandleGet(w http.ResponseWriter, r *http.Request) {
    w.WriteHeader(http.StatusOK)
}
"#,
            ours: r#"package handlers

func HandleGet(w http.ResponseWriter, r *http.Request) {
    w.WriteHeader(http.StatusOK)
}

func HandlePost(w http.ResponseWriter, r *http.Request) {
    w.WriteHeader(http.StatusCreated)
}
"#,
            theirs: r#"package handlers

func HandleGet(w http.ResponseWriter, r *http.Request) {
    w.WriteHeader(http.StatusOK)
}

func HandleDelete(w http.ResponseWriter, r *http.Request) {
    w.WriteHeader(http.StatusNoContent)
}
"#,
        },
    ];

    let mut total_weave_clean = 0;
    let mut total_git_clean = 0;
    let total_scenarios = scenarios.len();

    for scenario in &scenarios {
        print!("  {:<50}", scenario.name);

        // Run weave merge
        let start = Instant::now();
        let weave_result = entity_merge(
            scenario.base,
            scenario.ours,
            scenario.theirs,
            scenario.file_path,
        );
        let weave_time = start.elapsed();

        // Run git-style merge (line-level via diffy)
        let start = Instant::now();
        let git_result = diffy::merge(scenario.base, scenario.ours, scenario.theirs);
        let git_time = start.elapsed();

        let weave_clean = weave_result.is_clean();
        let git_clean = git_result.is_ok();

        if weave_clean {
            total_weave_clean += 1;
        }
        if git_clean {
            total_git_clean += 1;
        }

        let status = match (weave_clean, git_clean) {
            (true, false) => "WEAVE WINS",
            (true, true) => "both clean",
            (false, true) => "git wins",
            (false, false) => "both conflict",
        };

        println!(
            "weave: {:>5}us ({:<9} {}) | git: {:>5}us ({}) | {}",
            weave_time.as_micros(),
            if weave_clean { "clean" } else { "CONFLICT" },
            weave_result.stats.confidence(),
            git_time.as_micros(),
            if git_clean { "clean" } else { "CONFLICT" },
            status,
        );
    }

    println!("\n--- Summary ---");
    println!(
        "weave: {}/{} clean merges ({:.0}%)",
        total_weave_clean,
        total_scenarios,
        total_weave_clean as f64 / total_scenarios as f64 * 100.0,
    );
    println!(
        "git:   {}/{} clean merges ({:.0}%)",
        total_git_clean,
        total_scenarios,
        total_git_clean as f64 / total_scenarios as f64 * 100.0,
    );

    let improvement = total_weave_clean as i32 - total_git_clean as i32;
    if improvement > 0 {
        println!(
            "\nweave resolved {} additional merge(s) that git could not.",
            improvement,
        );
        println!(
            "False conflict reduction: {:.0}%",
            if total_scenarios > total_git_clean {
                improvement as f64 / (total_scenarios - total_git_clean) as f64 * 100.0
            } else {
                0.0
            },
        );
    }

    Ok(())
}

struct Scenario {
    name: &'static str,
    #[allow(dead_code)]
    description: &'static str,
    file_path: &'static str,
    base: &'static str,
    ours: &'static str,
    theirs: &'static str,
}
