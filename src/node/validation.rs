use std::fmt::Display;

use anyhow::{Result, bail};
use camino::Utf8Path;

use super::GenerateNodePackageOptions;

pub(crate) fn validate_generate_options(options: &GenerateNodePackageOptions) -> Result<()> {
    validate_output_dir(options)?;
    validate_library_input_path(options)?;
    validate_manifest_path(options)
}

fn validate_output_dir(options: &GenerateNodePackageOptions) -> Result<()> {
    validate_non_empty_path(options.out_dir.as_ref(), "--out-dir")?;
    if options.out_dir.exists() && !options.out_dir.is_dir() {
        bail!(
            "--out-dir '{}' exists but is not a directory",
            options.out_dir
        );
    }

    Ok(())
}

fn validate_library_input_path(options: &GenerateNodePackageOptions) -> Result<()> {
    validate_non_empty_path(options.lib_source.as_ref(), "<LIB_SOURCE>")?;
    validate_existing_file(
        options.lib_source.as_ref(),
        format!(
            "built UniFFI cdylib '{}' does not exist",
            options.lib_source
        ),
        format!("built UniFFI cdylib '{}' is not a file", options.lib_source),
    )?;
    if !uniffi_bindgen::is_cdylib(&options.lib_source) {
        bail!(
            "built UniFFI cdylib '{}' must end in .so, .dylib, or .dll",
            options.lib_source
        );
    }

    Ok(())
}

fn validate_manifest_path(options: &GenerateNodePackageOptions) -> Result<()> {
    let Some(manifest_path) = options.manifest_path.as_deref() else {
        return Ok(());
    };

    validate_non_empty_path(manifest_path, "--manifest-path")?;
    validate_existing_file(
        manifest_path,
        format!("manifest path '{}' does not exist", manifest_path),
        format!(
            "--manifest-path '{}' must point to a Cargo.toml file",
            manifest_path
        ),
    )
}

fn validate_non_empty_path(path: &Utf8Path, label: &str) -> Result<()> {
    if path.as_str().trim().is_empty() {
        bail!("{label} cannot be empty");
    }

    Ok(())
}

fn validate_existing_file(
    path: &Utf8Path,
    missing_message: impl Display,
    invalid_type_message: impl Display,
) -> Result<()> {
    if !path.exists() {
        bail!("{missing_message}");
    }
    if !path.is_file() {
        bail!("{invalid_type_message}");
    }

    Ok(())
}
