use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

pub type BuildTarget = String;
pub type BuildTargetPattern = String;

pub type Rules = HashMap<BuildTarget, BuildRule>;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct CommonBuildRule {
    /// The name of the build rule, which must be unique within a build file.
    pub name: String,
    /// The build rule's dependencies, expressed as a list of build targets.
    #[serde(default)]
    pub deps: Vec<BuildTarget>,
    /// List of build target patterns that identify the build rules that can
    /// include this rule as a dependency, for example, by listing it in their
    /// deps or exported_deps attributes. For more information, see visibility.
    #[serde(default)]
    pub visibility: Vec<BuildTargetPattern>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "buck.type")]
#[serde(rename_all = "snake_case")]
pub enum BuildRuleType {
    RustBinary(RustBinaryRule),
    RustLibrary(RustLibraryRule),
    RustTest(RustTestRule),
    PrebuiltRustLibrary(PrebuiltRustLibraryRule),
    #[serde(other)]
    Other,
}

impl BuildRuleType {
    fn krate_mut(&mut self) -> Option<&mut String> {
        match self {
            BuildRuleType::RustBinary(binary) => Some(&mut binary.krate),
            BuildRuleType::RustLibrary(library) => Some(&mut library.krate),
            BuildRuleType::RustTest(test) => Some(&mut test.krate),
            BuildRuleType::PrebuiltRustLibrary(preb) => Some(&mut preb.krate),
            _ => None,
        }
    }

    pub fn is_supported(&self) -> bool {
        match self {
            BuildRuleType::RustBinary(..)
            | BuildRuleType::RustLibrary(..)
            | BuildRuleType::RustTest(..) => true,
            _ => false,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            BuildRuleType::RustBinary(..) => "rust_binary",
            BuildRuleType::RustLibrary(..) => "rust_library",
            BuildRuleType::RustTest(..) => "rust_test",
            BuildRuleType::PrebuiltRustLibrary(..) => "prebuilt_rust_library",
            _ => "<unrecognized>",
        }
    }

    pub fn is_library(&self) -> bool {
        match self {
            BuildRuleType::RustLibrary(..) | BuildRuleType::PrebuiltRustLibrary(..) => true,
            _ => false,
        }
    }

    #[rustfmt::skip]
    pub fn crate_root(&self) -> Option<&Path> {
        let (srcs, crate_root, krate) = match self {
            | BuildRuleType::RustBinary(RustBinaryRule { ref srcs, ref crate_root, ref krate, ..})
            | BuildRuleType::RustLibrary(RustLibraryRule { ref srcs, ref crate_root, ref krate, ..})
            | BuildRuleType::RustTest(RustTestRule { ref srcs, ref crate_root, ref krate, ..}) => {
                (srcs, crate_root, krate)
            },
            _ => None?,
        };

        if !crate_root.as_os_str().is_empty() {
            Some(crate_root)
        } else {
            let default_filename = if self.is_library() {
                "lib.rs"
            } else {
                "main.rs"
            };
            let crate_filename = &format!("{}.rs", krate);

            let shortest_path_for_filename = |file_name: &str| srcs
                .iter()
                .filter_map(|x| x.file_name().map(|y| (x.components().count(), x, y)))
                .filter(|(_, _, name)| *name == file_name)
                .min_by_key(|(count, _, _)| *count)
                .map(|(_, path, _)| path.as_path());

            shortest_path_for_filename(default_filename).or_else(||
                shortest_path_for_filename(crate_filename))
        }
    }
}

/// A rust_binary() rule builds a native executable from the supplied set of
/// Rust source files and dependencies.
///
/// If you invoke a build with the check flavor, then Buck will invoke rustc to
/// check the code (typecheck, produce warnings, etc), but won't generate an
/// executable code. When applied to binaries it produces no output; for
/// libraries it produces metadata for consumers of the library. When building
/// with check, extra compiler flags from the rust.rustc_check_flags are added
/// to the compiler's command line options, to allow for extra warnings, etc.
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct RustBinaryRule {
    /// The set of Rust source files to be compiled by this rule.
    ///
    /// One of the source files is the root module of the crate. By default
    /// this is lib.rs for libraries, main.rs for executables, or the crate's
    /// name with .rs appended. This can be overridden with the crate_root rule
    /// parameter.
    srcs: Vec<PathBuf>,
    /// These are passed to `rustc` with --cfg feature="{feature}", and can be
    /// used in the code with #[cfg(feature = "{feature}")].
    features: Vec<String>,
    /// The set of additional compiler flags to pass to `rustc`.
    rustc_flags: Vec<String>,
    /// The set of additional flags to pass to the linker.
    linker_flags: Vec<String>,
    #[serde(rename = "crate")]
    /// Set the generated crate name (for libraries) or executable name (for
    /// binaries), independent of the rule name. Defaults to the rule name.
    krate: String,
    /// Set the name of the top-level source file for the crate, which can be
    /// used to override the default (see srcs).
    crate_root: PathBuf,
    /// Determines whether to build and link this rule's dependencies statically
    /// or dynamically. Can be either static, static_pic or shared.
    link_style: LinkStyle,
    /// Set the "rpath" in the executable when using a shared link style.
    rpath: bool,
    /// List of build targets that identify tests that exercise this target.
    tests: Vec<BuildTarget>,
    /// Set of license files for this library. To get the list of license files
    /// for a given build rule and all of its dependencies, you can use buck
    /// query.
    licenses: Vec<String>,
    /// Set of arbitrary strings which allow you to annotate a build rule with
    /// tags that can be searched for over an entire dependency tree using buck
    /// query attrfilter.
    labels: Vec<String>,
}

impl Default for RustBinaryRule {
    fn default() -> Self {
        RustBinaryRule {
            rpath: true,
            srcs: Default::default(),
            features: Default::default(),
            rustc_flags: Default::default(),
            linker_flags: Default::default(),
            krate: Default::default(),
            crate_root: Default::default(),
            link_style: Default::default(),
            tests: Default::default(),
            licenses: Default::default(),
            labels: Default::default(),
        }
    }
}

/// A rust_library() rule builds a native library from the supplied set of Rust
/// source files and dependencies.
///
/// If you invoke a build with the check flavor, then Buck will invoke rustc to
/// check the code (typecheck, produce warnings, etc), but won't generate an
/// executable code. When applied to binaries it produces no output; for
/// libraries it produces metadata for consumers of the library. When building
/// with check, extra compiler flags from the rust.rustc_check_flags are added
/// to the compiler's command line options, to allow for extra warnings, etc.
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct RustLibraryRule {
    /// The set of Rust source files to be compiled by this rule.
    ///
    /// One of the source files is the root module of the crate. By default
    /// this is lib.rs for libraries, main.rs for executables, or the crate's
    /// name with .rs appended. This can be overridden with the crate_root rule
    /// parameter.
    srcs: Vec<PathBuf>,
    /// These are passed to `rustc` with --cfg feature="{feature}", and can be
    /// used in the code with #[cfg(feature = "{feature}")].
    features: Vec<String>,
    /// The set of additional compiler flags to pass to `rustc`.
    rustc_flags: Vec<String>,
    #[serde(rename = "crate")]
    /// Set the generated crate name (for libraries) or executable name (for
    /// binaries), independent of the rule name. Defaults to the rule name.
    krate: String,
    /// Set the name of the top-level source file for the crate, which can be
    /// used to override the default (see srcs).
    crate_root: PathBuf,
    /// Controls how a library should be linked.
    preferred_linkage: PreferredLinkage,
    /// List of build targets that identify tests that exercise this target.
    tests: Vec<BuildTarget>,
    /// Set of license files for this library. To get the list of license files
    /// for a given build rule and all of its dependencies, you can use buck
    /// query.
    licenses: Vec<String>,
    /// Set of arbitrary strings which allow you to annotate a build rule with
    /// tags that can be searched for over an entire dependency tree using buck
    /// query attrfilter.
    labels: Vec<String>,
}

/// A rust_test() rule builds a Rust test native executable from the supplied
/// set of Rust source files and dependencies and runs this test.
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct RustTestRule {
    /// The set of Rust source files to be compiled by this rule.
    ///
    /// One of the source files is the root module of the crate. By default
    /// this is lib.rs for libraries, main.rs for executables, or the crate's
    /// name with .rs appended. This can be overridden with the crate_root rule
    /// parameter.
    srcs: Vec<PathBuf>,
    /// Use the standard test framework. If this is set to false, then the
    /// result is a normal executable which requires a `main()`, etc. It is
    /// still expected to accept the same command-line parameters and
    /// produce the same output as the test framework.
    framework: bool,
    /// These are passed to `rustc` with --cfg feature="{feature}", and can be
    /// used in the code with #[cfg(feature = "{feature}")].
    features: Vec<String>,
    /// The set of additional compiler flags to pass to `rustc`.
    rustc_flags: Vec<String>,
    #[serde(rename = "crate")]
    /// Set the generated crate name (for libraries) or executable name (for
    /// binaries), independent of the rule name. Defaults to the rule name.
    krate: String,
    /// Set the name of the top-level source file for the crate, which can be
    /// used to override the default (see srcs).
    crate_root: PathBuf,
    /// Determines whether to build and link this rule's dependencies statically
    /// or dynamically. Can be either static, static_pic or shared.
    link_style: LinkStyle,
    /// Set of license files for this library. To get the list of license files
    /// for a given build rule and all of its dependencies, you can use buck
    /// query.
    licenses: Vec<String>,
    /// Set of arbitrary strings which allow you to annotate a build rule with
    /// tags that can be searched for over an entire dependency tree using buck
    /// query attrfilter.
    labels: Vec<String>,
}

impl Default for RustTestRule {
    fn default() -> Self {
        RustTestRule {
            framework: true,
            srcs: Default::default(),
            features: Default::default(),
            rustc_flags: Default::default(),
            krate: Default::default(),
            crate_root: Default::default(),
            link_style: Default::default(),
            licenses: Default::default(),
            labels: Default::default(),
        }
    }
}

/// A prebuilt_rust_library() specifies a pre-built Rust crate, and any
/// dependencies it may have on other crates (typically also prebuilt).
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct PrebuiltRustLibraryRule {
    /// Path to the precompiled Rust crate - typically of the form
    /// 'libfoo.rlib', or 'libfoo-abc123def456.rlib' if it has symbol
    /// versioning metadata.
    rlib: PathBuf,
    /// Set the generated crate name (for libraries) or executable name (for
    /// binaries), independent of the rule name. Defaults to the rule name.
    #[serde(rename = "crate")]
    krate: String,
    /// Set of license files for this library. To get the list of license files
    /// for a given build rule and all of its dependencies, you can use buck
    /// query.
    licenses: Vec<String>,
    /// Set of arbitrary strings which allow you to annotate a build rule with
    /// tags that can be searched for over an entire dependency tree using buck
    /// query attrfilter.
    labels: Vec<String>,
}

/// Determines whether to build and link this rule's dependencies statically or
/// dynamically.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum LinkStyle {
    Static,
    StaticPic,
    Shared,
}

impl Default for LinkStyle {
    fn default() -> Self {
        LinkStyle::Static
    }
}

/// Controls how a library should be linked.
#[derive(Serialize, Deserialize, Debug)]
pub enum PreferredLinkage {
    /// The library will be linked based on its dependents `link_style`.
    Any,
    /// The library will be always be linked as a shared library.
    Shared,
    /// The library will be linked as a static library.
    /// Note: since shared libraries re-export its dependencies, depending on
    /// multiple shared libraries which themselves have overlapping static
    /// dependencies will cause duplicate symbols.
    Static,
}

impl Default for PreferredLinkage {
    fn default() -> Self {
        PreferredLinkage::Any
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct BuildRule {
    #[serde(rename = "buck.base_path")]
    pub base_path: PathBuf,
    #[serde(rename = "buck.direct_dependencies")]
    pub direct_dependencies: Vec<BuildTarget>,
    #[serde(flatten)]
    pub common: CommonBuildRule,
    #[serde(flatten)]
    pub typ: BuildRuleType,
}

pub fn buck_command(dir: impl AsRef<Path>, rule: impl AsRef<str>) -> Command {
    let mut cmd = Command::new("buck");
    cmd.arg("query")
        .arg(format!("deps({})", rule.as_ref()))
        .arg("--output-attributes")
        .arg(".*")
        .current_dir(dir.as_ref());
    cmd
}

pub fn from_bytes(bytes: &[u8]) -> Result<Rules, serde_json::Error> {
    let mut rules: Rules = serde_json::from_slice(bytes)?;

    // Adjust default `crate` field to rule name, if applies
    for rule in rules.values_mut() {
        if let Some(krate) = rule.typ.krate_mut().filter(|x| x.is_empty()) {
            *krate = rule.common.name.clone();
        }
    }

    Ok(rules)
}

pub fn query_rules(dir: impl AsRef<Path>, rule: impl AsRef<str>) -> Result<Rules, failure::Error> {
    let output = buck_command(dir, rule).output()?;
    if !output.status.success() {
        return Err(BuckError(
            output.status,
            String::from_utf8_lossy(&output.stderr).to_string(),
        )
        .into());
    }

    from_bytes(&output.stdout).map_err(|x| x.into())
}

pub fn buck_root(cwd: impl AsRef<Path>) -> Result<PathBuf, failure::Error> {
    let output = Command::new("buck")
        .arg("root")
        .current_dir(cwd.as_ref())
        .output()?;

    if output.status.success() {
        Ok(PathBuf::from(String::from_utf8(output.stdout)?.trim()))
    } else {
        Err(BuckError(
            output.status,
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))?
    }
}

#[derive(Debug)]
pub struct BuckError(std::process::ExitStatus, String);

impl fmt::Display for BuckError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error running Buck ({}): {}", self.0, self.1)
    }
}

impl std::error::Error for BuckError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_root_default_src() {
        let rule = BuildRuleType::RustBinary(RustBinaryRule {
            srcs: vec![PathBuf::from("src/main.rs")],
            ..Default::default()
        });
        assert_eq!(rule.crate_root(), Some(Path::new("src/main.rs")));

        let rule = BuildRuleType::RustBinary(RustBinaryRule {
            srcs: vec![PathBuf::from("src/lib.rs")],
            ..Default::default()
        });
        assert_eq!(rule.crate_root(), None);

        let rule = BuildRuleType::RustBinary(RustBinaryRule {
            srcs: vec![PathBuf::from("src/mycrate.rs")],
            krate: String::from("mycrate"),
            ..Default::default()
        });
        assert_eq!(rule.crate_root(), Some(Path::new("src/mycrate.rs")));

        let rule = BuildRuleType::RustLibrary(RustLibraryRule {
            srcs: vec![PathBuf::from("src/lib.rs")],
            ..Default::default()
        });
        assert_eq!(rule.crate_root(), Some(Path::new("src/lib.rs")));

        let rule = BuildRuleType::RustLibrary(RustLibraryRule {
            srcs: vec![PathBuf::from("src/main.rs")],
            ..Default::default()
        });
        assert_eq!(rule.crate_root(), None);

        let rule = BuildRuleType::RustLibrary(RustLibraryRule {
            srcs: vec![PathBuf::from("src/mycrate.rs")],
            krate: String::from("mycrate"),
            ..Default::default()
        });
        assert_eq!(rule.crate_root(), Some(Path::new("src/mycrate.rs")));
    }

    #[test]
    fn crate_root_override() {
        let rule = BuildRuleType::RustBinary(RustBinaryRule {
            srcs: vec![PathBuf::from("src/main.rs"), PathBuf::from("override.rs")],
            crate_root: PathBuf::from("override.rs"),
            ..Default::default()
        });
        // TODO: Check if crate_root is in srcs?
        assert_eq!(rule.crate_root(), Some(Path::new("override.rs")));
    }

    #[test]
    fn crate_root_shortest_path() {
        let rule = BuildRuleType::RustBinary(RustBinaryRule {
            srcs: vec![
                PathBuf::from("some/inner/main.rs"),
                PathBuf::from("main.rs"),
                PathBuf::from("some/main.rs"),
            ],
            ..Default::default()
        });
        assert_eq!(rule.crate_root(), Some(Path::new("main.rs")));
    }

    #[test]
    fn crate_root_preference() {
        let rule = BuildRuleType::RustLibrary(RustLibraryRule {
            srcs: vec![
                PathBuf::from("lib.rs"),
                PathBuf::from("some/lib.rs"),
                PathBuf::from("mycrate.rs"),
            ],
            krate: String::from("mycrate"),
            crate_root: PathBuf::from("some/lib.rs"),
            ..Default::default()
        });
        assert_eq!(rule.crate_root(), Some(Path::new("some/lib.rs")));

        let rule = BuildRuleType::RustLibrary(RustLibraryRule {
            srcs: vec![PathBuf::from("mycrate.rs"), PathBuf::from("some/lib.rs")],
            krate: String::from("mycrate"),
            ..Default::default()
        });
        assert_eq!(rule.crate_root(), Some(Path::new("some/lib.rs")));
    }
}
