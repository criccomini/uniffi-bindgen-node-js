use anyhow::{Result, bail};

use super::GenerateNodePackageOptions;

pub(crate) fn validate_generate_options(options: &GenerateNodePackageOptions) -> Result<()> {
    validate_output_dir(options)?;
    validate_library_input_path(options)?;
    validate_manifest_path(options)
}

fn validate_output_dir(options: &GenerateNodePackageOptions) -> Result<()> {
    if options.out_dir.as_str().trim().is_empty() {
        bail!("--out-dir cannot be empty");
    }
    if options.out_dir.exists() && !options.out_dir.is_dir() {
        bail!(
            "--out-dir '{}' exists but is not a directory",
            options.out_dir
        );
    }

    Ok(())
}

fn validate_library_input_path(options: &GenerateNodePackageOptions) -> Result<()> {
    if options.lib_source.as_str().trim().is_empty() {
        bail!("<LIB_SOURCE> cannot be empty");
    }
    if !options.lib_source.exists() {
        bail!(
            "built UniFFI cdylib '{}' does not exist",
            options.lib_source
        );
    }
    if !options.lib_source.is_file() {
        bail!("built UniFFI cdylib '{}' is not a file", options.lib_source);
    }
    if !uniffi_bindgen::is_cdylib(&options.lib_source) {
        bail!(
            "built UniFFI cdylib '{}' must end in .so, .dylib, or .dll",
            options.lib_source
        );
    }

    Ok(())
}

fn validate_manifest_path(options: &GenerateNodePackageOptions) -> Result<()> {
    let Some(manifest_path) = options.manifest_path.as_ref() else {
        return Ok(());
    };

    if manifest_path.as_str().trim().is_empty() {
        bail!("--manifest-path cannot be empty");
    }
    if !manifest_path.exists() {
        bail!("manifest path '{}' does not exist", manifest_path);
    }
    if !manifest_path.is_file() {
        bail!(
            "--manifest-path '{}' must point to a Cargo.toml file",
            manifest_path
        );
    }

    Ok(())
}
