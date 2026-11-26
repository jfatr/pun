use std::path::Path;

use pun_analyse::{Project, run};
use serde_json::Value;

// A chain through a nullable property after a `?->` reports the property fetch on `Box|null` and types the fetch and
// the call `int|null`, while the same chain without `?->` types them `int` after the error (#92): PHPStan 2.3.5's errors
// on chains.php at level 8, sorted
#[test]
fn a_nullsafe_chain_through_a_nullable_property() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/nullsafe-nullable-chains");
    let oracle: Value =
        serde_json::from_slice(&std::fs::read(root.join("expected-phpstan.json")).unwrap())
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
    let mut project = Project::new(vec![root.join("chains.php")], Vec::new());
    project.level = 8;
    let prefix = format!("{}/", root.display());
    let mut got: Vec<_> = (run(&project).unwrap().into_iter())
        .map(|e| {
            let file = e.file.strip_prefix(&prefix).unwrap_or(&e.file).to_owned();
            (file, e.line, e.identifier, e.message)
        })
        .collect();
    want.sort();
    got.sort();
    assert_eq!(got, want);
}
