use super::{load, Format};
use serde_json::Value;
use std::{env, fs, path::Path, process::Command};

#[test]
#[ignore = "requires the yaml-test-suite data checkout used in CI"]
fn upstream_yaml_fixtures() {
    let root = env::var_os("YAML_TEST_SUITE").expect("set YAML_TEST_SUITE to the fixture checkout");
    let root = Path::new(&root);
    let files = Command::new("git")
        .current_dir(root)
        .args(["ls-files", "--", "*/in.yaml"])
        .output()
        .unwrap();
    assert!(files.status.success(), "could not list YAML fixtures");
    let files = String::from_utf8(files.stdout).unwrap();
    let mut compared = 0;
    for file in files.lines() {
        let id = file.strip_suffix("/in.yaml").unwrap();
        let dir = root.join(id);
        let yaml = fs::read_to_string(root.join(file)).unwrap();
        let keys = crate::datafile::keys("input.yaml", &yaml).unwrap();
        let values = load(&yaml, Format::Yaml);
        if dir.join("error").exists() {
            assert!(
                keys.is_err() && values.is_none(),
                "{id}: expected invalid YAML"
            );
            continue;
        }
        // The event parser accepts these, but their mappings repeat a key.
        if matches!(id, "2JQS" | "X38W") {
            assert!(keys.unwrap_err().contains("duplicate key"), "{id}");
            assert!(values.is_none(), "{id}");
            continue;
        }
        assert!(keys.is_ok(), "{id}: {keys:?}");
        let json = dir.join("in.json");
        if json.exists() {
            compared += 1;
            // Upstream's JSON omits these YAML tags. A merge must preserve them.
            let tagged = [
                "2XXW", "565N", "5TYM", "6CK3", "6WLZ", "7FWL", "9WXW", "C4HZ", "CC74", "CUP7",
                "J7PZ", "M5C3", "P76L", "UGM3", "Z67P", "Z9M4",
            ];
            if tagged.contains(&id) {
                assert!(values.is_none(), "{id}: tag lost in JSON conversion");
            } else {
                let json = fs::read_to_string(json).unwrap();
                let expected = serde_json::Deserializer::from_str(&json)
                    .into_iter::<Value>()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                assert_eq!(values.as_ref(), Some(&expected), "{id}");
            }
        }
    }
    assert_eq!((files.lines().count(), compared), (402, 279));
}
