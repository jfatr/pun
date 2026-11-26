use std::path::Path;

use pun_analyse::{Project, run};
use serde_json::Value;

// An anonymous class is a subtype of the class it extends and the interfaces it implements, so it is accepted where
// they are declared (#95), while a class it does not extend is still rejected: PHPStan 2.3.5's errors on
// subtypes.php at level 10
#[test]
fn anonymous_classes_are_accepted_as_their_parents() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/anonymous-subtypes");
    let oracle: Value =
        serde_json::from_slice(&std::fs::read(root.join("expected-phpstan.json")).unwrap())
            .unwrap();
    let want: Vec<_> = (oracle["errors"].as_array().unwrap().iter())
        .map(|e| {
            (
                e["line"].as_i64().unwrap(),
                e["identifier"].as_str().unwrap().to_owned(),
                e["message"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let mut project = Project::new(vec![root.join("subtypes.php")], Vec::new());
    project.level = 10;
    let got: Vec<_> = (run(&project).unwrap().into_iter())
        .map(|e| (e.line, e.identifier, e.message))
        .collect();
    assert_eq!(got, want);
}
