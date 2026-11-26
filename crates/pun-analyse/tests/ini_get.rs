use std::path::Path;

use pun_analyse::{Project, run};
use serde_json::Value;

// PHPStan's IniGetReturnTypeExtension: `ini_get()` of an option PHP always defines is a `string` or a
// `numeric-string`, any other option keeps `string|false` (#102): PHPStan 2.3.5's errors on ini.php at level 10 on
// PHP 8.5
#[test]
fn ini_get_of_a_known_option_is_a_string() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ini-get");
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
    let mut project = Project::new(vec![root.join("ini.php")], Vec::new());
    project.level = 10;
    let got: Vec<_> = (run(&project).unwrap().into_iter())
        .map(|e| (e.line, e.identifier, e.message))
        .collect();
    assert_eq!(got, want);
}
