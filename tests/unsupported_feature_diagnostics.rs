mod support;

use insta::assert_snapshot;
use uniffi_bindgen::BindingGenerator;

use self::support::{
    component_from_webidl, component_with_namespace, generation_settings, generator,
};

#[test]
fn write_bindings_reports_all_unsupported_uniffi_features() {
    let generator = generator();
    let settings = generation_settings("unsupported-uniffi-features");
    let component = component_from_webidl(
        r#"
        [External="other-crate"]
        typedef enum ExternalThing;

        [Custom]
        typedef string Url;

        namespace example {
            ExternalThing read_external();
            Url parse_url(string value);
        };

        callback interface Logger {
            [Async] void write(string message);
        };
        "#,
    );

    let error = generator
        .write_bindings(&settings, &[component])
        .expect_err("unsupported v1 features should be rejected");

    assert_snapshot!(
        error.to_string(),
        @r#"
    unsupported UniFFI features for Node bindings v1:
    - external types are not supported in v1: ExternalThing
    - custom types are not supported in v1: Url
    "#
    );
}

#[test]
fn update_component_configs_rejects_commonjs_output_with_generator_error() {
    let generator = generator();
    let settings = generation_settings("unsupported-commonjs");
    let mut components = vec![component_with_namespace("example")];
    components[0].config.module_format = Some("commonjs".to_string());

    let error = generator
        .update_component_configs(&settings, &mut components)
        .expect_err("CommonJS output should be rejected");

    assert_snapshot!(
        error.to_string(),
        @"node bindings v1 are ESM-only; CommonJS output is not supported"
    );
}
