use anyhow::Result;
use uniffi_bindgen::{BindingGenerator, Component, GenerationSettings};

#[derive(Debug, Clone, Default)]
pub struct NodeBindingGenerator;

impl NodeBindingGenerator {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Clone, Default)]
pub struct NodeBindingGeneratorConfig;

impl BindingGenerator for NodeBindingGenerator {
    type Config = NodeBindingGeneratorConfig;

    fn new_config(&self, _root_toml: &toml::value::Value) -> Result<Self::Config> {
        Ok(Self::Config::default())
    }

    fn update_component_configs(
        &self,
        _settings: &GenerationSettings,
        _components: &mut Vec<Component<Self::Config>>,
    ) -> Result<()> {
        Ok(())
    }

    fn write_bindings(
        &self,
        _settings: &GenerationSettings,
        _components: &[Component<Self::Config>],
    ) -> Result<()> {
        Ok(())
    }
}
