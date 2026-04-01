use anyhow::{Result, bail};

use super::GenerateNodePackageOptions;

pub(crate) fn validate_generate_options(options: &GenerateNodePackageOptions) -> Result<()> {
    validate_library_input_path(options)
}

fn validate_library_input_path(options: &GenerateNodePackageOptions) -> Result<()> {
    if options.lib_source.as_str().trim().is_empty() {
        bail!("lib_source cannot be empty");
    }
    if !options.lib_source.exists() {
        bail!("library source '{}' does not exist", options.lib_source);
    }
    if !options.lib_source.is_file() {
        bail!("library source '{}' is not a file", options.lib_source);
    }
    if !uniffi_bindgen::is_cdylib(&options.lib_source) {
        bail!(
            "library source '{}' is not a supported cdylib (.so, .dylib, or .dll)",
            options.lib_source
        );
    }

    Ok(())
}
