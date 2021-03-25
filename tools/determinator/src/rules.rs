// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Custom rules for the target determinator.
//!
//! By default, the target determinator follows a simple set of rules:
//! * Every changed path is matched to its nearest package, and that package is marked changed.
//! * Cargo builds are simulated against the old and new package graphs, and any packages with
//!   different results are marked changed.
//! * The affected set is found through observing simulated Cargo builds and doing a reverse map.
//!
//! However, there is often a need to customize these rules, for example to:
//! * ignore certain files
//! * build everything if certain files or packages have changed
//! * add *virtual dependencies* that Cargo may not know about: if a package changes, also consider
//!   certain other packages as changed.
//!
//! These custom behaviors can be specified through *determinator rules*.
//!
//! There are two sorts of determinator rules:
//! * **Path rules** match on changed paths, and are applied **in order**, before regular matches.
//! * **Package rules** match based on changed packages, and are applied as required until
//!   exhausted (i.e. a fixpoint is reached).
//!
//! Determinator rules are a configuration file format and can be read from a TOML file.
//!
//! # Default path rules
//!
//! The determinator ships with a set of default path rules for common files such as `.gitignore`
//! and `Cargo.lock`. These rules are applied *after* custom rules, so custom rules matching the
//! same paths can override them.
//!
//! The default rules can be [viewed here](DeterminatorRules::DEFAULT_RULES_TOML).
//!
//! To disable default rules entirely, set at the top level:
//!
//! ```toml
//! use-default-rules = false
//! ```
//!
//! # Examples for path rules
//!
//! To ignore all files named `README.md` and `README.tpl`, and skip all further processing:
//!
//! ```toml
//! [[path-rule]]
//! # Globs are implemented using globset: https://docs.rs/globset/0.4
//! globs = ["**/README.md", "**/README.tpl"]
//! mark-changed = []
//! # "skip" is the default for post-rule, so it can be omitted.
//! post-rule = "skip"
//! ```
//!
//! To mark a package changed if a file in a different directory changes, but also continue to
//! use the standard algorithm to match paths to their nearest package:
//!
//! ```toml
//! [[path-rule]]
//! # Note that globs are relative to the root of the workspace.
//! globs = ["cargo-guppy/src/lib.rs"]
//! # Workspace packages are specified through their names.
//! mark-changed = ["cargo-compare"]
//! # "skip-rules" means that cargo-guppy/src/lib.rs will also match cargo-guppy.
//! post-rule = "skip-rules"
//! ```
//!
//! To build everything if a special file changes:
//!
//! ```toml
//! [[path-rule]]
//! name = "rust-toolchain"
//! mark-changed = "all"
//! ```
//!
//! To apply multiple rules to a file, say `CODE_OF_CONDUCT.md`:
//!
//! ```toml
//! [[path-rule]]
//! globs = ["CODE_OF_CONDUCT.md", "CONTRIBUTING.md"]
//! mark-changed = ["cargo-guppy"]
//! # "fallthrough" means further rules are applied as well.
//! post-rule = "fallthrough"
//!
//! [[path-rule]]
//! globs = ["CODE_OF_CONDUCT.md"]
//! mark-changed = ["guppy"]
//! ```
//!
//! # Examples for package rules
//!
//! To add a "virtual dependency" that Cargo may not know about:
//!
//! ```toml
//! [[package-rule]]
//! on-affected = ["fixtures"]
//! mark-changed = ["guppy-cmdlib"]
//! ```
//!
//! To build everything if a package changes.
//!
//! ```toml
//! [[package-rule]]
//! on-affected = ["guppy-benchmarks"]
//! mark-changed = "all"
//! ```

use crate::errors::RulesError;
use globset::{Glob, GlobSet, GlobSetBuilder};
use guppy::graph::{PackageGraph, PackageMetadata, PackageSet, Workspace};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Rules for the target determinator.
///
/// This forms a configuration file format that can be read from a TOML file.
///
/// For more about determinator rules, see [the module-level documentation](index.html).
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeterminatorRules {
    /// Whether to use the default rules, as specified by `DEFAULT_RULES_TOML` and `default_rules`.
    ///
    /// This is true by default.
    #[serde(default = "default_true", rename = "use-default-rules")]
    use_default_rules: bool,

    /// A list of rules that each changed file path is matched against.
    #[serde(default, rename = "path-rule")]
    pub path_rules: Vec<PathRule>,

    /// A list of rules that each affected package is matched against.
    ///
    /// Sometimes, dependencies between workspace packages aren't expressed in Cargo.tomls. The
    /// packages here act as "virtual dependencies" for the determinator.
    #[serde(default, rename = "package-rule")]
    pub package_rules: Vec<PackageRule>,
}

/// The `Default` impl is the set of custom rules used by the determinator if
/// [`set_rules`](crate::Determinator::set_rules) isn't called. It is an empty set of determinator
/// rules, with `use_default_rules` set to true. This means that if `set_rules` isn't
/// called, the only rules in effect are the default ones.
impl Default for DeterminatorRules {
    fn default() -> Self {
        Self {
            use_default_rules: true,
            path_rules: vec![],
            package_rules: vec![],
        }
    }
}

#[inline]
fn default_true() -> bool {
    true
}

/// A hack that lets the contents of default-rules.toml be included.
macro_rules! doc_comment {
    ($doc:expr, $($t:tt)*) => (
        #[doc = $doc]
        $($t)*
    );
}

impl DeterminatorRules {
    /// Deserializes determinator rules from the given TOML string.
    pub fn parse(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    doc_comment! {
        concat!("\
Contains the default rules in a TOML file format.

The default rules included with this copy of the determinator are:

```toml
", include_str!("../default-rules.toml"), "\
```

The latest version of the default rules is available
[on GitHub](https://github.com/facebookincubator/cargo-guppy/blob/main/tools/determinator/default-rules.toml).
"),
        pub const DEFAULT_RULES_TOML: &'static str = include_str!("../default-rules.toml");
    }

    /// Returns the default rules.
    ///
    /// These rules are applied *after* any custom rules, so they can be overridden by custom rules.
    pub fn default_rules() -> &'static DeterminatorRules {
        static DEFAULT_RULES: Lazy<DeterminatorRules> = Lazy::new(|| {
            DeterminatorRules::parse(DeterminatorRules::DEFAULT_RULES_TOML)
                .expect("default rules should parse")
        });

        &*DEFAULT_RULES
    }
}

/// Path-based rules for the determinator.
///
/// These rules customize the behavior of the determinator based on changed paths.
///
/// # Examples
///
/// ```toml
/// [[path-rule]]
/// globs = ["**/README.md", "**/README.tpl"]
/// mark-changed = ["guppy"]
/// ```
///
/// For more examples, see [the module-level documentation](index.html).
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct PathRule {
    /// The globs to match against.
    ///
    /// A changed path matches a rule if it matches any of the globs on this list.
    ///
    /// # Examples
    ///
    /// In TOML format, this is specified as [`globset`](https://docs.rs/globset/0.4) globs:
    ///
    /// ```toml
    /// globs = ["foo", "**/bar/*.rs"]
    /// ```
    pub globs: Vec<String>,

    /// The set of packages to mark as changed.
    ///
    /// # Examples
    ///
    /// In TOML format, this may be the string `"all"` to cause all packages to be marked changed:
    ///
    /// ```toml
    /// mark-changed = "all"
    /// ```
    ///
    /// Alternatively, `mark-changed` may be an array of workspace package names:
    ///
    /// ```toml
    /// mark-changed = ["guppy", "determinator"]
    /// ```
    #[serde(with = "mark_changed_impl")]
    pub mark_changed: DeterminatorMarkChanged,

    /// The operation to perform after applying the rule. Set to "skip" by default.
    #[serde(default)]
    pub post_rule: DeterminatorPostRule,
}

/// The operation to perform after applying the rule.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum DeterminatorPostRule {
    /// Skip all further processing of this path.
    ///
    /// This is the default.
    ///
    /// # Examples
    ///
    /// In TOML format, specified as the string `"skip"`:
    ///
    /// ```toml
    /// post-rule = "skip"
    /// ```
    Skip,

    /// Skip rule processing but continue attempting to match the changed path to the nearest
    /// package name.
    ///
    /// # Examples
    ///
    /// In TOML format, specified as the string `"skip-rules"`:
    ///
    /// ```toml
    /// post-rule = "skip-rules"
    /// ```
    SkipRules,

    /// Continue to apply further rules.
    ///
    /// # Examples
    ///
    /// In TOML format, specified as the string `"fallthrough"`:
    ///
    /// ```toml
    /// post-rule = "fallthrough"
    /// ```
    Fallthrough,
}

impl Default for DeterminatorPostRule {
    fn default() -> Self {
        DeterminatorPostRule::Skip
    }
}

/// Package-based rules for the determinator.
///
/// These rules customize the behavior of the determinator based on affected packages, and can be
/// used to insert "virtual dependencies" that Cargo may not be aware of.
///
/// # Examples
///
/// ```toml
/// [[package-rules]]
/// on-affected = ["determinator"]
/// mark-changed = ["guppy"]
/// ```
///
/// For more examples, see [the module-level documentation](index.html).
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct PackageRule {
    /// The package names to match against.
    ///
    /// If any of the packages in this list is affected, the given packages will be marked changed.
    ///
    /// # Examples
    ///
    /// In TOML format, specified as an array of workspace package names:
    ///
    /// ```toml
    /// on-affected = ["target-spec", "guppy"]
    /// ```
    pub on_affected: Vec<String>,

    /// The set of packages to mark as changed.
    ///
    /// # Examples
    ///
    /// In TOML format, this may be the string `"all"`:
    ///
    /// ```toml
    /// mark-changed = "all"
    /// ```
    ///
    /// or an array of workspace package names:
    ///
    /// ```toml
    /// mark-changed = ["guppy", "determinator"]
    /// ```
    #[serde(with = "mark_changed_impl")]
    pub mark_changed: DeterminatorMarkChanged,
}

/// The set of packages to mark as changed.
///
/// # Examples
///
/// In TOML format, this may be the string `"all"` to cause all packages to be marked changed:
///
/// ```toml
/// mark-changed = "all"
/// ```
///
/// Alternatively, `mark-changed` may be an array of workspace package names:
///
/// ```toml
/// mark-changed = ["guppy", "determinator"]
/// ```
///
/// For more examples, see [the module-level documentation](index.html).
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", untagged)]
pub enum DeterminatorMarkChanged {
    /// Mark the workspace packages with the given names as changed.
    ///
    /// This may be empty:
    ///
    /// ```toml
    /// mark-changed = []
    /// ```
    Packages(Vec<String>),

    /// Mark the entire tree as changed. Skip over all further processing and return the entire
    /// workspace as affected.
    ///
    /// This is most useful for global files that affect the environment.
    All,
}

/// The result of matching a file path against a determinator.
///
/// Returned by `Determinator::match_path`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PathMatch {
    /// The path matched a rule, causing everything to be rebuilt.
    RuleMatchedAll,
    /// The path matched a rule and ancestor-based matching was not followed.
    ///
    /// This will not be returned if the matched rule caused ancestor-based matching to happen.
    RuleMatched(RuleIndex),
    /// The path was matched to a package through inspecting the parent directories of each path.
    AncestorMatched,
    /// The path wasn't matched to a rule or a nearby package, causing everything to be rebuilt.
    NoMatches,
}

/// The index of a rule.
///
/// Used in `PathMatch` and while returning errors.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuleIndex {
    /// The custom path rule at this index.
    CustomPath(usize),
    /// The default path rule at this index.
    DefaultPath(usize),
    /// The package rule at this index.
    ///
    /// All package rules are custom: there are no default package rules.
    Package(usize),
}

impl fmt::Display for RuleIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RuleIndex::CustomPath(index) => write!(f, "custom path rule {}", index),
            RuleIndex::DefaultPath(index) => write!(f, "default path rule {}", index),
            RuleIndex::Package(index) => write!(f, "package rule {}", index),
        }
    }
}

// ---
// Private types
// ---

/// Internal version of determinator rules.
#[derive(Clone, Debug)]
pub(crate) struct RulesImpl<'g> {
    pub(crate) path_rules: Vec<PathRuleImpl<'g>>,
    pub(crate) package_rules: Vec<PackageRuleImpl<'g>>,
}

impl<'g> RulesImpl<'g> {
    pub(crate) fn new(
        graph: &'g PackageGraph,
        options: &DeterminatorRules,
    ) -> Result<Self, RulesError> {
        let workspace = graph.workspace();

        let custom_path_rules = options
            .path_rules
            .iter()
            .enumerate()
            .map(|(idx, rule)| (RuleIndex::CustomPath(idx), rule));

        let default_path_rules = if options.use_default_rules {
            let default_rules = DeterminatorRules::default_rules();
            default_rules.path_rules.as_slice()
        } else {
            &[]
        };

        let default_path_rules = default_path_rules
            .iter()
            .enumerate()
            .map(|(idx, rule)| (RuleIndex::DefaultPath(idx), rule));

        // Default rules come after custom ones.
        let path_rules = custom_path_rules
            .chain(default_path_rules)
            .map(
                |(
                    rule_index,
                    PathRule {
                        globs,
                        mark_changed,
                        post_rule,
                    },
                )| {
                    // Convert the globs to a globset.
                    let mut builder = GlobSetBuilder::new();
                    for glob in globs {
                        let glob = Glob::new(glob)
                            .map_err(|err| RulesError::glob_parse(rule_index, err))?;
                        builder.add(glob);
                    }

                    let glob_set = builder
                        .build()
                        .map_err(|err| RulesError::glob_parse(rule_index, err))?;

                    // Convert workspace paths to packages.
                    let mark_changed = MarkChangedImpl::new(&workspace, mark_changed)
                        .map_err(|err| RulesError::resolve_ref(rule_index, err))?;

                    Ok(PathRuleImpl {
                        rule_index,
                        glob_set,
                        mark_changed,
                        post_rule: *post_rule,
                    })
                },
            )
            .collect::<Result<Vec<_>, _>>()?;

        let package_rules = options
            .package_rules
            .iter()
            .enumerate()
            .map(
                |(
                    rule_index,
                    PackageRule {
                        on_affected,
                        mark_changed,
                    },
                )| {
                    let rule_index = RuleIndex::Package(rule_index);
                    let on_affected = graph
                        .resolve_workspace_names(on_affected)
                        .map_err(|err| RulesError::resolve_ref(rule_index, err))?;
                    let mark_changed = MarkChangedImpl::new(&workspace, mark_changed)
                        .map_err(|err| RulesError::resolve_ref(rule_index, err))?;
                    Ok(PackageRuleImpl {
                        on_affected,
                        mark_changed,
                    })
                },
            )
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            path_rules,
            package_rules,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PathRuleImpl<'g> {
    pub(crate) rule_index: RuleIndex,
    pub(crate) glob_set: GlobSet,
    pub(crate) mark_changed: MarkChangedImpl<'g>,
    pub(crate) post_rule: DeterminatorPostRule,
}

#[derive(Clone, Debug)]
pub(crate) struct PackageRuleImpl<'g> {
    pub(crate) on_affected: PackageSet<'g>,
    pub(crate) mark_changed: MarkChangedImpl<'g>,
}

#[derive(Clone, Debug)]
pub(crate) enum MarkChangedImpl<'g> {
    All,
    Packages(Vec<PackageMetadata<'g>>),
}

impl<'g> MarkChangedImpl<'g> {
    fn new(
        workspace: &Workspace<'g>,
        mark_changed: &DeterminatorMarkChanged,
    ) -> Result<Self, guppy::Error> {
        match mark_changed {
            DeterminatorMarkChanged::Packages(names) => Ok(MarkChangedImpl::Packages(
                workspace.members_by_names(names)?,
            )),
            DeterminatorMarkChanged::All => Ok(MarkChangedImpl::All),
        }
    }
}

mod mark_changed_impl {
    use super::*;
    use serde::{de::Error, Deserializer, Serializer};

    pub fn serialize<S>(
        mark_changed: &DeterminatorMarkChanged,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match mark_changed {
            DeterminatorMarkChanged::Packages(names) => names.serialize(serializer),
            DeterminatorMarkChanged::All => "all".serialize(serializer),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DeterminatorMarkChanged, D::Error>
    where
        D: Deserializer<'de>,
    {
        let d = MarkChangedDeserialized::deserialize(deserializer)?;
        match d {
            MarkChangedDeserialized::String(s) => match s.as_str() {
                "all" => Ok(DeterminatorMarkChanged::All),
                other => Err(D::Error::custom(format!(
                    "unknown string for mark-changed: {}",
                    other,
                ))),
            },
            MarkChangedDeserialized::VecString(strings) => {
                Ok(DeterminatorMarkChanged::Packages(strings))
            }
        }
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum MarkChangedDeserialized {
        String(String),
        VecString(Vec<String>),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let s = r#"[[path-rule]]
        globs = ["all/*"]
        mark-changed = "all"
        post-rule = "fallthrough"

        [[path-rule]]
        globs = ["all/1/2/*"]
        mark-changed = ["c"]
        post-rule = "skip-rules"

        [[path-rule]]
        globs = ["none/**/test", "foo/bar"]
        mark-changed = []

        [[package-rule]]
        on-affected = ["foo"]
        mark-changed = ["wat"]

        [[package-rule]]
        on-affected = ["test1"]
        mark-changed = "all"
        "#;

        let expected = DeterminatorRules {
            use_default_rules: true,
            path_rules: vec![
                PathRule {
                    globs: vec!["all/*".to_owned()],
                    mark_changed: DeterminatorMarkChanged::All,
                    post_rule: DeterminatorPostRule::Fallthrough,
                },
                PathRule {
                    globs: vec!["all/1/2/*".to_owned()],
                    mark_changed: DeterminatorMarkChanged::Packages(vec!["c".to_owned()]),
                    post_rule: DeterminatorPostRule::SkipRules,
                },
                PathRule {
                    globs: vec!["none/**/test".to_owned(), "foo/bar".to_owned()],
                    mark_changed: DeterminatorMarkChanged::Packages(vec![]),
                    post_rule: DeterminatorPostRule::Skip,
                },
            ],
            package_rules: vec![
                PackageRule {
                    on_affected: vec!["foo".to_string()],
                    mark_changed: DeterminatorMarkChanged::Packages(vec!["wat".to_string()]),
                },
                PackageRule {
                    on_affected: vec!["test1".to_string()],
                    mark_changed: DeterminatorMarkChanged::All,
                },
            ],
        };

        assert_eq!(
            DeterminatorRules::parse(s),
            Ok(expected),
            "parse() result matches"
        );
    }

    #[test]
    fn parse_empty() {
        let expected = DeterminatorRules::default();

        assert_eq!(
            DeterminatorRules::parse(""),
            Ok(expected),
            "parse_empty() returns default"
        );
    }

    #[test]
    fn parse_bad() {
        let bads = &[
            // **************
            // General errors
            // **************

            // unrecognized section
            r#"[[foo]]
            bar = "baz"
            "#,
            // unrecognized section
            r#"[foo]
            bar = "baz"
            "#,
            //
            // **********
            // Path rules
            // **********
            //
            // unrecognized key
            r#"[[path-rule]]
            globs = ["a/b"]
            mark-changed = []
            foo = "bar"
            "#,
            // globs is not a list
            r#"[[path-rule]]
            globs = "x"
            mark-changed = []
            "#,
            // glob list doesn't have a string
            r#"[[path-rule]]
            globs = [123, "a/b"]
            mark-changed = []
            "#,
            // rule totally missing
            r#"[[path-rule]]
            "#,
            // globs missing
            r#"[[path-rule]]
            mark-changed = "all"
            "#,
            // mark-changed missing
            r#"[[path-rule]]
            globs = ["a/b"]
            "#,
            // mark-changed is an invalid string
            r#"[[path-rule]]
            globs = ["a/b"]
            mark-changed = "foo"
            "#,
            // mark-changed is not a string or list
            r#"[[path-rule]]
            globs = ["a/b"]
            mark-changed = 123
            "#,
            // mark-changed is not a list of strings
            r#"[[path-rule]]
            globs = ["a/b"]
            mark-changed = [123, "abc"]
            "#,
            // post-rule is invalid
            r#"[[path-rule]]
            globs = ["a/b"]
            mark-changed = []
            post-rule = "abc"
            "#,
            // post-rule is not a string
            r#"[[path-rule]]
            globs = ["a/b"]
            mark-changed = "all"
            post-rule = []
            "#,
            //
            // *************
            // Package rules
            // *************
            //
            // unrecognized key
            r#"[[package-rule]]
            on-affected = ["foo"]
            mark-changed = []
            foo = "bar"
            "#,
            // on-affected is not a list
            r#"[[package-rule]]
            on-affected = "foo"
            mark-changed = []
            "#,
            // on-affected doesn't contain strings
            r#"[[package-rule]]
            on-affected = ["foo", 123]
            mark-changed = []
            "#,
            // mark-changed is not a string or list
            r#"[[package-rule]]
            on-affected = ["foo"]
            mark-changed = 123
            "#,
            // mark-changed is not a list of strings
            r#"[[package-rule]]
            on-affected = ["foo", 123]
            mark-changed = ["bar", 456]
            "#,
            // mark-changed is an invalid string
            r#"[[package-rule]]
            on-affected = ["foo"]
            mark-changed = "bar"
            "#,
            // on-affected is missing
            r#"[[package-rule]]
            mark-changed = "all"
            "#,
            // mark-changed is missing
            r#"[[package-rule]]
            on-affected = ["foo"]
            "#,
        ];

        for &bad in bads {
            let res = DeterminatorRules::parse(bad);
            if res.is_ok() {
                panic!(
                    "parsing should have failed but succeeded:\n\
                     input = {}\n\
                     output: {:?}\n",
                    bad, res
                );
            }
        }
    }
}
