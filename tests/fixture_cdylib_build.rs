mod support;

use self::support::{build_fixture_cdylib, remove_dir_all};

#[test]
fn builds_callback_fixture_cdylib_in_an_isolated_workspace() {
    let built_fixture = build_fixture_cdylib("callbacks");
    let expected_filename = format!(
        "{}{}.{}",
        std::env::consts::DLL_PREFIX,
        built_fixture.crate_name,
        std::env::consts::DLL_EXTENSION
    );

    assert!(
        built_fixture.manifest_path.is_file(),
        "expected copied fixture manifest at {}",
        built_fixture.manifest_path
    );
    assert!(
        built_fixture.library_path.is_file(),
        "expected built cdylib at {}",
        built_fixture.library_path
    );
    assert_eq!(built_fixture.crate_name, "fixture_callbacks");
    assert_eq!(
        built_fixture.library_path.file_name(),
        Some(expected_filename.as_str()),
        "unexpected cdylib filename: {}",
        built_fixture.library_path
    );

    remove_dir_all(&built_fixture.workspace_dir);
}
