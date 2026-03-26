mod support;

use self::support::{build_slatedb_cdylib, remove_dir_all};

#[test]
fn builds_local_slatedb_uniffi_cdylib_without_writing_artifacts_into_the_repo() {
    let built = build_slatedb_cdylib();

    assert!(
        built.library_path.exists(),
        "expected SlateDB cdylib to exist at {}",
        built.library_path
    );
    assert_eq!(built.crate_name, "slatedb_uniffi");
    assert!(
        built
            .library_path
            .as_std_path()
            .starts_with(built.target_dir.as_std_path()),
        "expected SlateDB cdylib {} to be built under temp target dir {}",
        built.library_path,
        built.target_dir
    );

    remove_dir_all(&built.target_dir);
}
