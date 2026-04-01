use anyhow::{Result, bail};
use uniffi_bindgen::Component;

use crate::node_v2::config::NodeBindingGeneratorConfig;

pub(crate) fn select_component(
    components: Vec<Component<NodeBindingGeneratorConfig>>,
    crate_name: Option<&str>,
) -> Result<Component<NodeBindingGeneratorConfig>> {
    let available_crate_names = components
        .iter()
        .map(|component| component.ci.crate_name().to_string())
        .collect::<Vec<_>>();
    match crate_name {
        Some(crate_name) => {
            let mut matching_components = components
                .into_iter()
                .filter(|component| component.ci.crate_name() == crate_name)
                .collect::<Vec<_>>();

            match matching_components.len() {
                1 => Ok(matching_components.remove(0)),
                0 => bail!(
                    "no UniFFI component for crate '{}' was found in the library; available crate names: {}",
                    crate_name,
                    available_crate_names.join(", ")
                ),
                count => bail!(
                    "expected exactly one UniFFI component for crate '{}', found {}",
                    crate_name,
                    count
                ),
            }
        }
        None => match components.len() {
            0 => bail!("no UniFFI components were found in the library"),
            1 => Ok(components
                .into_iter()
                .next()
                .expect("single component should be present")),
            _ => bail!(
                "the library contains multiple UniFFI components; re-run with --crate-name to select one. available crate names: {}",
                available_crate_names.join(", ")
            ),
        },
    }
}

pub(crate) fn normalize_crate_name_selector(crate_name: &str) -> String {
    crate_name.replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    use uniffi_bindgen::interface::ComponentInterface;

    fn test_component(crate_name: &str, namespace: &str) -> Component<NodeBindingGeneratorConfig> {
        Component {
            ci: ComponentInterface::from_webidl(
                &format!("namespace {namespace} {{}};"),
                crate_name,
            )
            .expect("valid test UDL"),
            config: NodeBindingGeneratorConfig::default(),
        }
    }

    #[test]
    fn normalize_crate_name_selector_accepts_cargo_package_names() {
        assert_eq!(
            normalize_crate_name_selector("slatedb-uniffi"),
            "slatedb_uniffi"
        );
        assert_eq!(
            normalize_crate_name_selector("slatedb_uniffi"),
            "slatedb_uniffi"
        );
    }

    #[test]
    fn select_component_reports_available_crates() {
        let error = select_component(
            vec![
                test_component("first_crate", "first"),
                test_component("second_crate", "second"),
            ],
            Some("missing_crate"),
        )
        .expect_err("missing component should fail");

        assert!(
            error
                .to_string()
                .contains("available crate names: first_crate, second_crate"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn select_component_infers_single_component_without_selector() {
        let component = select_component(vec![test_component("only_crate", "example")], None)
            .expect("single component should be inferred");

        assert_eq!(component.ci.crate_name(), "only_crate");
    }

    #[test]
    fn select_component_requires_selector_for_multiple_components() {
        let error = select_component(
            vec![
                test_component("first_crate", "first"),
                test_component("second_crate", "second"),
            ],
            None,
        )
        .expect_err("multiple components should require a selector");

        assert!(
            error
                .to_string()
                .contains("available crate names: first_crate, second_crate"),
            "unexpected error: {error}"
        );
        assert!(
            error.to_string().contains("re-run with --crate-name"),
            "unexpected error: {error}"
        );
    }
}
