use std::path::Path;

use pun_analyse::{Project, run};
use serde_json::Value;

// A generic built from a narrowed `mixed` (after `if ($value)` or `!== null`) is accepted for `Box<int>` below level 9,
// as one built from plain `mixed` is, and reported with it at level 9 (#90): PHPStan 2.3.5's errors on boxes.php at
// levels 6 and 9, sorted
#[test]
fn a_generic_of_narrowed_mixed_is_checked_only_with_explicit_mixed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/narrowed-mixed-generics");
    for level in [6, 9] {
        let oracle: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("expected-level-{level}.json"))).unwrap(),
        )
        .unwrap();
        let mut want: Vec<_> = (oracle["errors"].as_array().unwrap().iter())
            .map(|e| {
                (
                    e["file"].as_str().unwrap().to_owned(),
                    e["line"].as_i64().unwrap(),
                    e["identifier"].as_str().unwrap().to_owned(),
                    e["message"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        let mut project = Project::new(vec![root.join("boxes.php")], Vec::new());
        project.level = level;
        let prefix = format!("{}/", root.display());
        let mut got: Vec<_> = (run(&project).unwrap().into_iter())
            .map(|e| {
                let file = e.file.strip_prefix(&prefix).unwrap_or(&e.file).to_owned();
                (file, e.line, e.identifier, e.message)
            })
            .collect();
        want.sort();
        got.sort();
        assert_eq!(got, want, "level {level}");
    }
}
