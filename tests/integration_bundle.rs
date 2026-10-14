//! The integration assets that Core ships (docs/adr/034): only shared skills remain after provider extraction.
//!
//! The release's packaging script is run by `mise run integrations:check`, not from a concurrent basic-suite test.

#![cfg(unix)]

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn package() -> PathBuf {
    let out = root().join("target/bundled-integrations");
    assert!(
        out.is_dir(),
        "run `mise run integrations:check` to build the package"
    );
    out
}

fn names(dir: &Path) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    found
}

#[test]
#[ignore = "`mise run integrations:check` packages the assets before running this test"]
fn the_core_package_holds_shared_skills_but_no_agent_specific_integrations() {
    let package = package();
    assert_eq!(names(&package), ["integrations", "skills"]);
    assert!(
        names(&package.join("integrations")).is_empty(),
        "agent integrations are shipped by their provider plugins"
    );

    let skills = names(&package.join("skills"));
    assert!(!skills.contains(&"README.md".to_string()), "{skills:?}");
    assert_eq!(
        skills,
        names(&root().join("skills"))
            .into_iter()
            .filter(|name| name != "README.md")
            .collect::<Vec<_>>()
    );
    for skill in skills {
        assert!(
            package.join("skills").join(&skill).join("SKILL.md").is_file(),
            "{skill}"
        );
    }
}
