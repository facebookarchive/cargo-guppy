// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{
    errors::RulesError,
    rules::{
        DeterminatorPostRule, DeterminatorRules, MarkChangedImpl, PathMatch, PathRuleImpl,
        RulesImpl,
    },
};
use globset::Candidate;
use guppy::{
    graph::{
        cargo::{CargoOptions, CargoSet},
        feature::{FeatureFilter, FeatureSet, StandardFeatures},
        DependencyDirection, PackageGraph, PackageMetadata, PackageSet, Workspace,
    },
    PackageId, Platform,
};
use itertools::Itertools;
use petgraph::{graphmap::GraphMap, Directed};
use rayon::prelude::*;
use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    path::Path,
};

/// Determine target dependencies from changed files and packages in a workspace.
///
/// For more about how the determinator works, see the [crate-level documentation](crate).
///
/// This struct has two lifetime parameters:
/// * `'g` stands for the lifetime of the new graph. The `DeterminatorSet` will be bound to this
///   lifetime.
/// * `'a` is the lifetime of the old graph, Cargo options, and changed paths. The `DeterminatorSet`
///   will not be bound to this lifetime.
#[derive(Clone, Debug)]
pub struct Determinator<'g, 'a> {
    old: &'a PackageGraph,
    new: &'g PackageGraph,
    rules: RulesImpl<'g>,
    cargo_options: Option<&'a CargoOptions<'a>>,
    old_features_only: Option<FeatureSet<'a>>,
    new_features_only: Option<FeatureSet<'g>>,
    changed_paths: Vec<&'a Path>,
}

impl<'g, 'a> Determinator<'g, 'a> {
    /// Creates a new instance of `Determinator` with old and new package graphs.
    pub fn new(old: &'a PackageGraph, new: &'g PackageGraph) -> Self {
        Self {
            old,
            new,
            rules: RulesImpl::new(new, &DeterminatorRules::default())
                .expect("default rules should parse"),
            cargo_options: None,
            old_features_only: None,
            new_features_only: None,
            changed_paths: vec![],
        }
    }

    /// Adds a list of changed paths. This list is used as a source of information for the
    /// determinator.
    ///
    /// This should consist of paths that are changed since the base revision, and should use the
    /// canonical separator for the platform (e.g. `/` on Unix platforms and `\` on Windows).
    ///
    /// [`Paths0`](crate::Paths0) in this crate provides a convenient way to handle null-separated
    /// paths as produced by source control systems.
    ///
    /// # Should you include untracked and ignored files?
    ///
    /// For comparisons against the working directory, one may or may not wish to include untracked
    /// files. A few points to consider:
    ///
    /// * If your code uses a library like [`datatest`](https://github.com/commure/datatest), simply
    ///   creating a file in the right place is enough to add a new test. If untracked files are
    ///   not included, the user may have to run `git add` before the determinator picks the change
    ///   up.
    /// * On the other hand, if a user wishes to include a new test in their PR, they're going to
    ///   have to run `git add` at some point anyway.
    /// * Some users may have untracked files lying around in their repository for long periods of
    ///   time, and those files may cause false positives.
    /// * Git makes it surprisingly hard to list out untracked files, requiring `git status
    ///   --porcelain -z` with some additional filtering on top to do so. `git diff` doesn't have
    ///   an option to list untracked files.
    /// * This is generally moot in CI, since those workflows will likely be comparing against a
    ///   commit.
    /// * In most cases, ignored files should not be considered by the determinator, since they
    ///   don't affect CI builds.
    ///
    /// On balance, only considering tracked files appears to be the right approach for most
    /// situations.
    pub fn add_changed_paths(&mut self, paths: impl IntoIterator<Item = &'a Path>) -> &mut Self {
        self.changed_paths.extend(paths);
        self
    }

    /// Returns what *would* happen if a given path was added to the changed set.
    ///
    /// This does not add any path to the changed set, but indicates what *would* happen if a path
    /// is added.
    ///
    /// This method may be used to ensure that all paths in a repository are covered by at least one
    /// rule if they don't match a package.
    ///
    /// `match_cb` is called for all package IDs that the path matches.
    pub fn match_path(
        &self,
        path: impl AsRef<Path>,
        match_cb: impl FnMut(&'g PackageId),
    ) -> PathMatch {
        process_path(
            path.as_ref(),
            &self.new.workspace(),
            &self.rules.path_rules,
            match_cb,
        )
    }

    /// Processes and configures determinator rules.
    ///
    /// Returns an error if the rules were invalid in some way.
    pub fn set_rules(&mut self, rules: &DeterminatorRules) -> Result<&mut Self, RulesError> {
        let rules = RulesImpl::new(self.new, rules)?;
        self.rules = rules;
        Ok(self)
    }

    /// Configures Cargo options.
    ///
    /// These options are used to determine if the build for a particular package has changed.
    ///
    /// If no options are specified, the default `CargoOptions`, as specified by
    /// `CargoOptions::new`, are used, with one exception: dev-dependencies are built by default.
    pub fn set_cargo_options(&mut self, cargo_options: &'a CargoOptions<'a>) -> &mut Self {
        self.cargo_options = Some(cargo_options);
        self
    }

    /// Returns the default Cargo options used by the determinator.
    ///
    /// These are the same as the defaults returned by [`CargoOptions::new`](CargoOptions::new),
    /// except:
    /// * dev-dependencies are enabled
    /// * the host and target platforms are set to the current platform
    pub fn default_cargo_options() -> CargoOptions<'static> {
        let mut options = CargoOptions::new();
        options
            .set_include_dev(true)
            .set_platform(Platform::current());
        options
    }

    /// Configures features-only packages that are used in build simulations.
    ///
    /// The packages and features will be used for feature unification. This is useful for
    /// pseudo-packages or workspace-hack packages, including those generated by tools like
    /// [Hakari](https://docs.rs/hakari).
    ///
    /// For more about `features_only`, see the documentation for [`CargoSet::new`](CargoSet::new).
    ///
    /// The package names are expected to be present in the new graph, but may not be present in the
    /// old `PackageGraph`.
    /// * If a package names isn't in the *new* graph, this method returns an error.
    /// * If a package names isn't in the *old* graph, it is ignored.
    pub fn set_features_only<'b>(
        &mut self,
        workspace_names: impl IntoIterator<Item = &'b str>,
        features: StandardFeatures,
    ) -> Result<&mut Self, guppy::Error> {
        let old_workspace = self.old.workspace();
        let mut old_names = vec![];
        let new_names: Vec<_> = workspace_names
            .into_iter()
            .inspect(|&name| {
                if old_workspace.contains_name(name) {
                    old_names.push(name);
                }
            })
            .collect();

        // Missing package name in new workspace => error.
        let new_features_only = self
            .new
            .resolve_workspace_names(new_names)?
            .to_feature_set(features);
        let old_features_only = self
            .old
            .resolve_workspace_names(old_names)
            .expect("old names were checked already")
            .to_feature_set(features);

        self.new_features_only = Some(new_features_only);
        self.old_features_only = Some(old_features_only);
        Ok(self)
    }

    /// Uses the old and new sets and the list of changed files to compute the list
    /// of projects that is affected.
    pub fn compute(&self) -> DeterminatorSet<'g> {
        let mut build_state = BuildState::new(self);

        // 1-2. Process every changed path.
        for path in &self.changed_paths {
            build_state = match build_state.process_path(path) {
                Some(build_state) => build_state,
                None => {
                    // The build state was discarded, which means that the entire workspace is
                    // changed and affected.
                    let path_changed_set = self.new.resolve_workspace();
                    let affected_set = path_changed_set.clone();
                    return DeterminatorSet {
                        path_changed_set,
                        // This is an empty set.
                        summary_changed_set: self.new.resolve_none(),
                        affected_set,
                    };
                }
            }
        }

        // 3. Construct the path changed set from the given IDs.
        let path_changed_set = self
            .new
            .resolve_ids(build_state.path_changed_ids.iter().copied())
            .expect("package IDs are all valid");

        // 4. Use build summaries as another source of changes.
        build_state.process_build_summaries();
        let summary_changed_set = self
            .new
            .resolve_ids(build_state.summary_changed_ids.iter().copied())
            .expect("package IDs are all valid");

        // 5. The affected set is the transitive closure of the graph constructed by looking at both
        // the build cache and Cargo rules.
        let affected_set = build_state.reverse_index.affected_closure(
            self.new,
            &build_state.path_changed_ids,
            &build_state.summary_changed_ids,
        );

        DeterminatorSet {
            path_changed_set,
            summary_changed_set,
            affected_set,
        }
    }
}

/// The result of a `Determinator` computation.
///
/// The lifetime `'g` is tied to the *new* `PackageGraph` passed to a `Determinator`.
#[derive(Clone, Debug)]
pub struct DeterminatorSet<'g> {
    /// The packages that were affected, directly or indirectly. This set is what most consumers
    /// care about.
    ///
    /// A package is in this set if it was marked changed due to a path or summaries changing, or if
    /// a simulated Cargo build or package rule indicated that it is affected.
    pub affected_set: PackageSet<'g>,

    /// The packages that were marked changed because a file changed.
    ///
    /// Either a file inside this package changed or a path rule was matched.
    pub path_changed_set: PackageSet<'g>,

    /// The packages that were marked changed becuase a simulated Cargo build's summary showed
    /// changes in dependencies.
    ///
    /// This does not include packages marked changed through a path. For example, if a path rule
    /// caused all packages to be marked changed, further steps aren't run and this set is empty.
    pub summary_changed_set: PackageSet<'g>,
}

// ---
// Private structures
// ---

#[derive(Debug)]
struct BuildState<'g, 'a, 'b> {
    determinator: &'b Determinator<'g, 'a>,
    path_changed_ids: HashSet<&'g PackageId>,
    summary_changed_ids: HashSet<&'g PackageId>,
    build_cache: CargoBuildCache<'g>,
    reverse_index: ReverseIndex<'g>,
}

impl<'g, 'a, 'b> BuildState<'g, 'a, 'b> {
    fn new(determinator: &'b Determinator<'g, 'a>) -> Self {
        let build_cache = CargoBuildCache::new(determinator);
        let reverse_index = ReverseIndex::new(determinator, &build_cache);
        Self {
            determinator,
            path_changed_ids: HashSet::new(),
            summary_changed_ids: HashSet::new(),
            build_cache,
            reverse_index,
        }
    }

    // A return value of None stands for all packages in the workspace changed.
    fn process_path(mut self, path: &Path) -> Option<Self> {
        let status = process_path(
            path,
            &self.determinator.new.workspace(),
            &self.determinator.rules.path_rules,
            |id| {
                self.path_changed_ids.insert(id);
            },
        );
        match status {
            PathMatch::RuleMatchedAll | PathMatch::NoMatches => None,
            PathMatch::RuleMatched(_) | PathMatch::AncestorMatched => Some(self),
        }
    }

    fn process_build_summaries(&mut self) {
        // For each workspace package, if its build summaries have changed mark it changed.
        let summary_changed_ids: Vec<_> = self
            .determinator
            .new
            .workspace()
            .par_iter_by_name()
            .filter_map(|(name, package)| {
                // Don't include packages already marked as changed through paths. (This is documented.)
                if !self.path_changed_ids.contains(package.id())
                    && self.build_summaries_changed(name, package)
                {
                    Some(package.id())
                } else {
                    None
                }
            })
            .collect();
        self.summary_changed_ids.extend(summary_changed_ids);
    }

    fn build_summaries_changed(&self, name: &str, package: PackageMetadata<'g>) -> bool {
        // Look up the package in the old metadata by path. (Workspace packages are uniquely
        // identified by both name and path -- this could be done by name as well).
        let old_workspace = self.determinator.old.workspace();
        let old_package = match old_workspace.member_by_name(name) {
            Ok(package) => package,
            Err(_) => {
                // Member not found: this is new or renamed.
                return true;
            }
        };

        let default_options = Determinator::default_cargo_options();
        let cargo_options = self.determinator.cargo_options.unwrap_or(&default_options);

        let default_features_only = self.determinator.old.feature_graph().resolve_none();
        let features_only = self
            .determinator
            .old_features_only
            .as_ref()
            .unwrap_or(&default_features_only);

        let old_result = BuildResult::new(old_package, cargo_options, features_only);
        let new_result = &self.build_cache.result_cache[package.id()];
        new_result.is_changed(&old_result, cargo_options)
    }
}

fn process_path<'g>(
    path: &Path,
    workspace: &Workspace<'g>,
    path_rules: &[PathRuleImpl<'g>],
    mut match_cb: impl FnMut(&'g PackageId),
) -> PathMatch {
    let candidate = Candidate::new(path);

    // 1. Apply any rules that match the path.
    for rule in path_rules {
        if rule.glob_set.is_match_candidate(&candidate) {
            // This glob matches this rule, so execute it.
            match &rule.mark_changed {
                MarkChangedImpl::Packages(packages) => {
                    for package in packages {
                        match_cb(package.id());
                    }
                }
                MarkChangedImpl::All => {
                    // Mark all packages changed.
                    return PathMatch::RuleMatchedAll;
                }
            }

            match &rule.post_rule {
                DeterminatorPostRule::Skip => {
                    // Skip all further processing for this path but continue reading other
                    // paths.
                    return PathMatch::RuleMatched(rule.rule_index);
                }
                DeterminatorPostRule::SkipRules => {
                    // Skip further rule processing but continue to step 2 to match to the
                    // nearest package.
                    break;
                }
                DeterminatorPostRule::Fallthrough => {
                    // Continue applying rules.
                    continue;
                }
            }
        }
    }

    // 2. Map the path to its nearest ancestor package.
    for ancestor in path.ancestors() {
        if let Ok(package) = workspace.member_by_path(ancestor) {
            match_cb(package.id());
            return PathMatch::AncestorMatched;
        }
    }

    // 3. If a file didn't match anything so far, rebuild everything.
    PathMatch::NoMatches
}

/// Stores a build cache of every package in a workspace.
#[derive(Debug)]
struct CargoBuildCache<'g> {
    result_cache: HashMap<&'g PackageId, BuildResult<'g>>,
}

impl<'g> CargoBuildCache<'g> {
    fn new(determinator: &Determinator<'g, '_>) -> Self {
        let default_options = Determinator::default_cargo_options();
        let cargo_options = determinator.cargo_options.unwrap_or(&default_options);

        let workspace = determinator.new.workspace();
        let default_features_only = determinator.new.feature_graph().resolve_none();
        let features_only = determinator
            .new_features_only
            .as_ref()
            .unwrap_or(&default_features_only);

        let result_cache: HashMap<_, _> = workspace
            .par_iter()
            .map(|package| {
                let id = package.id();
                let build_result = BuildResult::new(package, cargo_options, features_only);
                (id, build_result)
            })
            .collect();

        Self { result_cache }
    }
}

#[derive(Debug)]
struct BuildResult<'g> {
    none: CargoSet<'g>,
    default: CargoSet<'g>,
    all: CargoSet<'g>,
    // TODO: add support for more feature sets?
}

impl<'g> BuildResult<'g> {
    fn new(
        package: PackageMetadata<'g>,
        cargo_options: &CargoOptions<'_>,
        features_only: &FeatureSet<'g>,
    ) -> Self {
        let (none, (default, all)) = rayon::join(
            || {
                make_cargo_set(
                    &package,
                    StandardFeatures::None,
                    cargo_options,
                    features_only,
                )
            },
            || {
                rayon::join(
                    || {
                        make_cargo_set(
                            &package,
                            StandardFeatures::Default,
                            cargo_options,
                            features_only,
                        )
                    },
                    || {
                        make_cargo_set(
                            &package,
                            StandardFeatures::All,
                            cargo_options,
                            features_only,
                        )
                    },
                )
            },
        );

        Self { none, default, all }
    }

    /// Returns the unified set of workspace dependencies.
    fn unified_workspace_set(&self, workspace_set: &PackageSet<'g>) -> PackageSet<'g> {
        let target_set = self
            .all_cargo_sets()
            .map(|x| x.target_features().to_package_set())
            .fold1(|a, b| a.union(&b))
            .expect("at least one set");
        let host_set = self
            .all_cargo_sets()
            .map(|x| x.host_features().to_package_set())
            .fold1(|a, b| a.union(&b))
            .expect("at least one set");

        target_set.union(&host_set).intersection(workspace_set)
    }

    fn is_changed(&self, other: &BuildResult<'_>, cargo_options: &CargoOptions<'_>) -> bool {
        for (a, b) in self.all_cargo_sets().zip(other.all_cargo_sets()) {
            let a_summary = a
                .to_summary(cargo_options)
                .expect("custom platforms currently unsupported");
            let b_summary = b
                .to_summary(cargo_options)
                .expect("custom platforms currently unsupported");
            let diff = a_summary.diff(&b_summary);
            if diff.is_changed() {
                return true;
            }
        }
        false
    }

    fn all_cargo_sets<'a>(&'a self) -> impl Iterator<Item = &'a CargoSet<'g>> + 'a {
        std::iter::once(&self.none)
            .chain(std::iter::once(&self.default))
            .chain(std::iter::once(&self.all))
    }
}

fn make_cargo_set<'x>(
    package: &PackageMetadata<'x>,
    filter: impl FeatureFilter<'x>,
    cargo_options: &CargoOptions<'_>,
    features_only: &FeatureSet<'x>,
) -> CargoSet<'x> {
    let package_set = package.to_package_set();
    let initials = package_set.to_feature_set(filter);

    CargoSet::new(initials, features_only.clone(), cargo_options).expect("valid cargo options")
}

/// A reverse index of if a package is affected -> what else gets marked changed or affected.
#[derive(Debug)]
struct ReverseIndex<'g> {
    // None for the node type represents the "all packages" sentinel value.
    reverse_index: GraphMap<Option<&'g PackageId>, ReverseIndexEdge, Directed>,
}

/// Edges in the reverse index graph.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ReverseIndexEdge {
    /// This edge was added as a package rule. This always takes precedence over `CargoBuild`.
    PackageRule,
    /// This edge was added through the Cargo build cache.
    CargoBuild,
}

impl<'g> ReverseIndex<'g> {
    fn new(determinator: &Determinator<'g, '_>, build_cache: &CargoBuildCache<'g>) -> Self {
        let mut reverse_index = GraphMap::new();

        let workspace_set = determinator.new.resolve_workspace();

        // First, look at the result cache and add edges based on that.
        for (id, build_result) in &build_cache.result_cache {
            reverse_index.extend(
                build_result
                    .unified_workspace_set(&workspace_set)
                    .package_ids(DependencyDirection::Forward)
                    .map(|dep_id| (Some(dep_id), Some(*id), ReverseIndexEdge::CargoBuild)),
            );
        }

        // Now, look at all the package rules and add anything in them to the reverse index.
        // IMPORTANT: This comes later so that PackageRule edges overwrite CargoBuild edges.
        for package_rule in &determinator.rules.package_rules {
            for on_affected in package_rule
                .on_affected
                .package_ids(DependencyDirection::Forward)
            {
                match &package_rule.mark_changed {
                    MarkChangedImpl::Packages(packages) => {
                        // Add edges from on_affected to mark_changed.
                        reverse_index.extend(packages.iter().map(|package| {
                            (
                                Some(on_affected),
                                Some(package.id()),
                                ReverseIndexEdge::PackageRule,
                            )
                        }));
                    }
                    MarkChangedImpl::All => {
                        // Add an edge to the None/"all" sentinel value.
                        reverse_index.add_edge(
                            Some(on_affected),
                            None,
                            ReverseIndexEdge::PackageRule,
                        );
                    }
                }
            }
        }

        Self { reverse_index }
    }

    fn affected_closure(
        &self,
        package_graph: &'g PackageGraph,
        path_changed: &HashSet<&'g PackageId>,
        summary_changed: &HashSet<&'g PackageId>,
    ) -> PackageSet<'g> {
        // This is a *really* interesting DFS, in that there's one restriction: you can't follow
        // two CargoBuild edges consecutively. Also, in the initial set, path_changed allows
        // CargoBuild to be followed once while summary_changed doesn't allow it to be followed.

        #[derive(Copy, Clone, Debug, Eq, PartialEq)]
        enum FollowCargoBuild {
            Allowed,
            NotAllowed,
        }

        use FollowCargoBuild::*;

        // The order of what goes in the stack doesn't matter for correctness, but putting Allowed
        // at the end (and therefore popping it first) lowers the chance of an upgrade re-traversal.
        let mut stack: Vec<_> = summary_changed
            .iter()
            .map(|id| (*id, NotAllowed))
            .chain(path_changed.iter().map(|id| (*id, Allowed)))
            .collect();

        // Do a DFS with two maps, in case there are cycles (can happen with dev deps).
        let mut discovered = HashMap::new();
        let mut finished = HashSet::new();

        while let Some(&(id, follow)) = stack.last() {
            let push_neighbors = match discovered.entry(id) {
                Entry::Vacant(entry) => {
                    // First time visiting this node. Push neighbors, don't pop the stack.
                    entry.insert(follow);
                    true
                }
                Entry::Occupied(mut entry) => {
                    // This node was seen before.
                    match (entry.get(), follow) {
                        (NotAllowed, Allowed) => {
                            // Upgrade this node to Allowed and push neighbors.
                            entry.insert(follow);
                            true
                        }
                        _ => {
                            // Already been fully discovered or just NotAllowed -> NotAllowed, no
                            // point revisiting it.
                            false
                        }
                    }
                }
            };

            if push_neighbors {
                for (_, neighbor, &edge) in self.reverse_index.edges(Some(id)) {
                    if edge == ReverseIndexEdge::CargoBuild && follow == NotAllowed {
                        // Can't follow two consecutive CargoBuild edges.
                        continue;
                    }
                    match neighbor {
                        Some(neighbor) => {
                            let neighbor_follow = match edge {
                                ReverseIndexEdge::CargoBuild => NotAllowed,
                                ReverseIndexEdge::PackageRule => Allowed,
                            };

                            match (discovered.get(&neighbor), neighbor_follow) {
                                (None, _) => {
                                    // Node has not been discovered yet. Add it to the stack to
                                    // be visited.
                                    stack.push((neighbor, neighbor_follow))
                                }
                                (Some(NotAllowed), Allowed) => {
                                    // Node was previously discovered with NotAllowed but is
                                    // now discovered with Allowed. This is an upgrade. Add it to
                                    // the stack to be visited.
                                    stack.push((neighbor, neighbor_follow))
                                }
                                _ => {}
                            }
                        }
                        None => {
                            // Build everything, can just exit here.
                            return package_graph.resolve_workspace();
                        }
                    }
                }
            } else {
                stack.pop();
                finished.insert(id);
            }
        }

        // At the end of this process, finished contains all nodes discovered.
        package_graph
            .resolve_ids(finished.iter().copied())
            .expect("all IDs are valid")
    }
}
