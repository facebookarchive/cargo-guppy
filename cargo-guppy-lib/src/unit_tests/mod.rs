mod dep_helpers;
mod fixtures;
mod graph_tests;
mod reversed_tests;

#[derive(Clone, Copy, Debug)]
pub(crate) enum DepDirection {
    Forward,
    Reverse,
}
