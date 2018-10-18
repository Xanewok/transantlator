//! Main translation logic.
//!
//! There are key differences between Buck and Cargo when it comes to packaging.
//! One of them is that there can be only one [lib] per each Cargo.toml, whereas
//! Buck can define multiple `rust_library` rules for a given BUCK buildfile.
//! These should be further translated to Cargo workspaces to work around that.
//! Another difference is that integration tests can be only specified as a part
//! of a given package, whereas Buck allows to specify `tests` as a set of build
//! targets, potentially outside the given package.
//! In addition to that, unit test targets are implicit in Cargo but these are
//! explicitly generated as separate *-unittest rules in Buck.

// TODO:
// * Support licenses
// * Support features
// * Support test targets
// * Generate Cargo workspaces for multiple libraries in the same buildfile
// * Coalesce dependencies for each Buck build target

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use crate::buck::{BuildRule, BuildTarget};

// Not a const since format! needs a literal and doesn't work with const &str
macro_rules! toml_header {
    () => {
        r#"[package]
name = "{}"
version = "0.1.0"
authors = ["Example <author@example.com>"]
"#
    };
}

pub fn translate_rules<'a>(
    buck_root: &Path,
    rules: impl Iterator<Item = (&'a BuildTarget, &'a BuildRule)>,
) -> Result<(), failure::Error> {
    let mut rules_by_dir = HashMap::<_, Vec<_>>::new();

    for (target, rule) in rules {
        rules_by_dir
            .entry(&rule.base_path)
            .or_default()
            .push((target, rule));
    }

    eprintln!("rules_by_dir: {:#?}", rules_by_dir);

    for (base_dir, rules) in rules_by_dir {
        let contents = translate_buildfile(base_dir, &rules)?;

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(buck_root.join(base_dir).join("Cargo.toml"))?;

        file.write_all(contents.as_bytes())?;
    }

    Ok(())
}

pub fn translate_buildfile(
    dir: &Path,
    rules: &[(&BuildTarget, &BuildRule)],
) -> Result<String, failure::Error> {
    let libs: Vec<&BuildRule> = rules
        .iter()
        .map(|(_, r)| *r)
        .filter(|r| r.typ.is_library())
        .collect();
    let bins: Vec<&BuildRule> = rules
        .iter()
        .map(|(_, r)| *r)
        .filter(|r| r.typ.is_binary() && !r.typ.is_test())
        .collect();
    // Reject multiple libraries in the same buildfile
    // TODO: Generate Cargo workspace for those?
    if libs.len() > 1 {
        let names = libs
            .iter()
            .map(|r| r.common.name.as_ref())
            .collect::<Vec<&str>>();
        return Err(failure::format_err!(
            "Multiple rust_library() in single buildfile is not yet supported ({}, {})",
            dir.display(),
            names.join(", ")
        ));
    }

    let default_bin = || {
        bins.iter()
            .find(|b| b.typ.crate_root().unwrap().file_name() == Some(&OsStr::new("main.rs")))
    };
    let default_rule = libs.get(0).or_else(default_bin).or_else(|| bins.get(0));
    let default_rule = default_rule.ok_or_else(|| failure::format_err!(
            "Couldn't find a fitting default Rule for buildfile {}",
            dir.display()
        )
    )?;

    let pkg_name = default_rule.typ.krate().unwrap();

    // FIXME: Use buffered writer
    let mut toml = format!(toml_header!(), pkg_name);

    if let Some(&lib) = libs.get(0) {
        toml.push_str("\n");
        toml.push_str("[lib]\n");
        toml.push_str(&format!(r#"name = "{}""#, lib.typ.krate().unwrap()));
        toml.push_str("\n");
        toml.push_str(&format!(
            r#"path = "{}""#,
            lib.typ.crate_root().unwrap().display()
        ));
        toml.push_str("\n");
    }

    for bin in bins {
        toml.push_str("\n");
        toml.push_str("[[bin]]\n");
        toml.push_str(&format!(r#"name = "{}""#, bin.typ.krate().unwrap()));
        toml.push_str("\n");
        toml.push_str(&format!(
            r#"path = "{}""#,
            bin.typ.crate_root().unwrap().display()
        ));
        toml.push_str("\n");
    }

    // TODO: For now reject code with unit tests having different deps than
    // bins/libs
    Ok(toml)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn reject_multiple_libs() {
        let input = r#"{
            "//dir:lib1" : {
                "buck.base_path" : "dir",
                "buck.direct_dependencies" : [],
                "buck.type" : "rust_library",
                "deps" : [],
                "name" : "lib1",
                "srcs" : [ "src/lib.rs" ],
                "visibility" : [ "PUBLIC" ]
            },
            "//dir:lib2" : {
                "buck.base_path" : "dir",
                "buck.direct_dependencies" : [],
                "buck.type" : "rust_library",
                "deps" : [],
                "name" : "lib2",
                "srcs" : [ "src/lib.rs" ],
                "visibility" : [ "PUBLIC" ]
            }
        }"#;
        let rules = crate::buck::from_bytes(input.as_bytes()).unwrap();
        let rules = rules.iter().collect::<Vec<(_, _)>>();

        let result = translate_buildfile(Path::new("dummy"), &rules);
        assert!(result.is_err());
    }

    #[test]
    fn translate_pkg_name_with_lib() -> Result<(), failure::Error> {
        let input = r#"{
            "//dir:bin1" : {
                "buck.base_path" : "dir",
                "buck.direct_dependencies" : [],
                "buck.type" : "rust_binary",
                "deps" : [],
                "name" : "bin1",
                "srcs" : [ "src/main.rs" ],
                "visibility" : [ "PUBLIC" ]
            },
            "//dir:lib1" : {
                "buck.base_path" : "dir",
                "buck.direct_dependencies" : [],
                "buck.type" : "rust_library",
                "deps" : [],
                "name" : "lib1",
                "srcs" : [ "src/lib.rs" ],
                "visibility" : [ "PUBLIC" ]
            },
            "//dir:aux_bin" : {
                "buck.base_path" : "dir",
                "buck.direct_dependencies" : [],
                "buck.type" : "rust_binary",
                "deps" : [],
                "name" : "aux_bin",
                "srcs" : [ "aux_bin.rs" ],
                "visibility" : [ "PUBLIC" ]
            }
        }"#;

        let rules = crate::buck::from_bytes(input.as_bytes()).unwrap();
        let rules: BTreeMap<_, _> = rules.into_iter().collect(); // deterministic
        let rules = rules.iter().collect::<Vec<(_, _)>>();
        assert_eq!(
            translate_buildfile(Path::new("dummy"), &rules)?,
            r#"[package]
name = "lib1"
version = "0.1.0"
authors = ["Example <author@example.com>"]

[lib]
name = "lib1"
path = "src/lib.rs"

[[bin]]
name = "aux_bin"
path = "aux_bin.rs"

[[bin]]
name = "bin1"
path = "src/main.rs"
"#
        );

        Ok(())
    }

    #[test]
    fn translate_pkg_name_with_bin() -> Result<(), failure::Error> {
        let input = r#"{
            "//dir:aux_bin" : {
                "buck.base_path" : "dir",
                "buck.direct_dependencies" : [],
                "buck.type" : "rust_binary",
                "deps" : [],
                "name" : "aux_bin",
                "srcs" : [ "aux_bin.rs" ],
                "visibility" : [ "PUBLIC" ]
            },
            "//dir:bin1" : {
                "buck.base_path" : "dir",
                "buck.direct_dependencies" : [],
                "buck.type" : "rust_binary",
                "deps" : [],
                "name" : "bin1",
                "srcs" : [ "src/main.rs" ],
                "visibility" : [ "PUBLIC" ]
            }
        }"#;

        let rules = crate::buck::from_bytes(input.as_bytes()).unwrap();
        let rules: BTreeMap<_, _> = rules.into_iter().collect(); // deterministic
        let rules = rules.iter().collect::<Vec<(_, _)>>();
        assert_eq!(
            translate_buildfile(Path::new("dummy"), &rules)?,
            r#"[package]
name = "bin1"
version = "0.1.0"
authors = ["Example <author@example.com>"]

[[bin]]
name = "aux_bin"
path = "aux_bin.rs"

[[bin]]
name = "bin1"
path = "src/main.rs"
"#
        );

        Ok(())
    }

    #[test]
    fn translate_lib() -> Result<(), failure::Error> {
        let input = r#"{
            "//dir:lib1" : {
                "buck.base_path" : "dir",
                "buck.direct_dependencies" : [],
                "buck.type" : "rust_library",
                "deps" : [],
                "name" : "lib1",
                "srcs" : [ "src/lib.rs" ],
                "visibility" : [ "PUBLIC" ]
            }
        }"#;
        let rules = crate::buck::from_bytes(input.as_bytes()).unwrap();
        let rules = rules.iter().collect::<Vec<(_, _)>>();
        assert_eq!(
            translate_buildfile(Path::new("dummy"), &rules[..1])?,
            r#"[package]
name = "lib1"
version = "0.1.0"
authors = ["Example <author@example.com>"]

[lib]
name = "lib1"
path = "src/lib.rs"
"#
        );

        Ok(())
    }
}
