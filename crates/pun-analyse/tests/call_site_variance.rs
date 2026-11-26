use std::path::Path;

use pun_analyse::{Project, run};
use serde_json::Value;

// A method template inferred from a callable returning `Items<covariant Animal>` keeps the call-site variance, so the
// result is accepted for `Items<Items<covariant Animal>>` and rejected for the invariant `Items<Items<Animal>>` (#91):
// PHPStan 2.3.5's errors on variance.php at level 6, sorted
#[test]
fn an_inferred_template_keeps_the_call_site_variance() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/call-site-variance");
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
    let mut project = Project::new(vec![root.join("variance.php")], Vec::new());
    project.level = 6;
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
