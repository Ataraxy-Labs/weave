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
    ];

    let mut total_weave_clean = 0;
    let mut total_git_clean = 0;
    let total_scenarios = scenarios.len();

    for scenario in &scenarios {
        print!("  {:<45}", scenario.name);

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
            "weave: {:>5}us ({}) | git: {:>5}us ({}) | {}",
            weave_time.as_micros(),
            if weave_clean { "clean" } else { "CONFLICT" },
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
