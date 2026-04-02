#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NodePackageSpec {
    pub(crate) package_name: String,
    pub(crate) library_name: String,
    pub(crate) node_engine: String,
    pub(crate) bundled_prebuilds: bool,
    pub(crate) manual_load: bool,
}
