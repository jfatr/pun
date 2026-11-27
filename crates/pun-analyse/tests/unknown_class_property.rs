use std::path::Path;

use pun_analyse::{Project, run};
use serde_json::Value;

// A property of an unknown class reads as `*ERROR*`, not `null` (#144): with `WP_Post` unknown, `! $post instanceof
// \WP_Post || 'publish' !== $post->post_status` is not always true, so the code after its early return is
// reachable. PHPStan 2.3.5's errors on page.php at level max
#[test]
fn a_property_of_an_unknown_class_is_an_error_type() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/unknown-class-property");
    let oracle: Value =
        serde_json::from_slice(&std::fs::read(root.join("expected-phpstan.json")).unwrap())
            .unwrap();
    let want: Vec<_> = (oracle["errors"].as_array().unwrap().iter())
        .map(|e| {
            (
                e["line"].as_i64().unwrap(),
                e["identifier"].as_str().unwrap().to_owned(),
                e["message"].as_str().unwrap().to_owned(),
                e["tip"].as_str().map(str::to_owned),
            )
        })
        .collect();
    let mut project = Project::new(vec![root.join("page.php")], Vec::new());
    project.level = 10;
    let got: Vec<_> = (run(&project).unwrap().into_iter())
        .map(|e| (e.line, e.identifier, e.message, e.tip))
        .collect();
    assert_eq!(got, want);
}
