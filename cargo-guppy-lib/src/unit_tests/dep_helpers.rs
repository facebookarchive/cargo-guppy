use crate::graph::{DependencyInfo, PackageMetadata};
use crate::unit_tests::fixtures::PackageDetails;

fn __from_metadata<'a>(dep: &DependencyInfo<'a>) -> &'a PackageMetadata {
    dep.from
}
fn __to_metadata<'a>(dep: &DependencyInfo<'a>) -> &'a PackageMetadata {
    dep.to
}
type DepToMetadata<'a> = fn(&DependencyInfo<'a>) -> &'a PackageMetadata;

/// Some of the messages are different based on whether we're testing forward deps or reverse
/// ones. For forward deps, we use the terms "known" for 'from' and "variable" for 'to'. For
/// reverse deps it's the other way round.
#[allow(dead_code)]
struct DirectionDesc<'a> {
    direction_desc: &'static str,
    known_desc: &'static str,
    variable_desc: &'static str,
    known_metadata: DepToMetadata<'a>,
    variable_metadata: DepToMetadata<'a>,
}

impl<'a> DirectionDesc<'a> {
    fn new(forward: bool) -> Self {
        if forward {
            Self::forward()
        } else {
            Self::reverse()
        }
    }

    fn forward() -> Self {
        Self {
            direction_desc: "forward",
            known_desc: "from",
            variable_desc: "to",
            known_metadata: __from_metadata as DepToMetadata<'a>,
            variable_metadata: __to_metadata as DepToMetadata<'a>,
        }
    }

    fn reverse() -> Self {
        Self {
            direction_desc: "reverse",
            known_desc: "to",
            variable_desc: "from",
            known_metadata: __to_metadata as DepToMetadata<'a>,
            variable_metadata: __from_metadata as DepToMetadata<'a>,
        }
    }

    fn known_metadata(&self, dep: &DependencyInfo<'a>) -> &'a PackageMetadata {
        (self.known_metadata)(dep)
    }

    fn variable_metadata(&self, dep: &DependencyInfo<'a>) -> &'a PackageMetadata {
        (self.variable_metadata)(dep)
    }
}

pub(crate) fn assert_deps_internal<'a>(
    forward: bool,
    known_details: &PackageDetails,
    expected_dep_ids: &[(&str, &str)],
    actual_deps: Vec<DependencyInfo<'a>>,
    msg: &str,
) {
    let desc = DirectionDesc::new(forward);

    // Compare (dep_name, resolved_name, id) triples.
    let expected_dep_ids: Vec<_> = expected_dep_ids
        .iter()
        .map(|(dep_name, id)| (*dep_name, dep_name.replace("-", "_"), *id))
        .collect();
    let mut actual_dep_ids: Vec<_> = actual_deps
        .iter()
        .map(|dep| {
            (
                dep.edge.dep_name(),
                dep.edge.resolved_name().to_string(),
                desc.variable_metadata(dep).id().repr.as_str(),
            )
        })
        .collect();
    actual_dep_ids.sort();
    assert_eq!(
        expected_dep_ids, actual_dep_ids,
        "{}: expected {} dependencies",
        msg, desc.direction_desc,
    );

    // Check that the dependency metadata returned is consistent with what we expect.
    let known_msg = format!(
        "{}: {} dependency edge {} this package",
        msg, desc.direction_desc, desc.known_desc
    );
    for actual_dep in &actual_deps {
        known_details.assert_metadata(desc.known_metadata(&actual_dep), &known_msg);
        // XXX maybe compare version requirements?
    }
}
