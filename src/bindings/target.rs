use anyhow::{Result, bail};

pub(crate) fn current_host_prebuild_target() -> Result<String> {
    let platform = current_node_platform()?;
    let arch = current_node_arch()?;

    if platform != "linux" {
        return Ok(format!("{platform}-{arch}"));
    }

    Ok(format!("{platform}-{arch}-{}", current_linux_libc()?))
}

pub(crate) fn current_host_bundled_library_file_name(library_name: &str) -> Result<String> {
    bundled_library_file_name(library_name, current_node_platform()?)
}

pub(crate) fn bundled_library_file_name(library_name: &str, platform: &str) -> Result<String> {
    match platform {
        "win32" => Ok(format!("{library_name}.dll")),
        "darwin" => Ok(format!("lib{library_name}.dylib")),
        "aix" | "android" | "freebsd" | "linux" | "openbsd" => Ok(format!("lib{library_name}.so")),
        other => bail!("unsupported Node platform for bundled library filename: {other}"),
    }
}

fn current_node_platform() -> Result<&'static str> {
    match std::env::consts::OS {
        "macos" => Ok("darwin"),
        "windows" => Ok("win32"),
        "linux" => Ok("linux"),
        "android" => Ok("android"),
        "aix" => Ok("aix"),
        "freebsd" => Ok("freebsd"),
        "openbsd" => Ok("openbsd"),
        other => bail!("unsupported host OS for Node bundled-prebuild resolution: {other}"),
    }
}

fn current_node_arch() -> Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("x64"),
        "x86" => Ok("ia32"),
        "aarch64" => Ok("arm64"),
        "arm" => Ok("arm"),
        "loongarch64" => Ok("loong64"),
        "powerpc64" => Ok("ppc64"),
        "riscv64" => Ok("riscv64"),
        "s390x" => Ok("s390x"),
        other => {
            bail!("unsupported host architecture for Node bundled-prebuild resolution: {other}")
        }
    }
}

fn current_linux_libc() -> Result<&'static str> {
    if cfg!(target_env = "gnu") {
        Ok("gnu")
    } else if cfg!(target_env = "musl") {
        Ok("musl")
    } else {
        bail!("unsupported Linux target environment for Node bundled-prebuild resolution")
    }
}

#[cfg(test)]
mod tests {
    use super::bundled_library_file_name;

    #[test]
    fn bundled_library_file_name_matches_platform_conventions() {
        assert_eq!(
            bundled_library_file_name("fixture", "darwin").expect("darwin filename"),
            "libfixture.dylib"
        );
        assert_eq!(
            bundled_library_file_name("fixture", "linux").expect("linux filename"),
            "libfixture.so"
        );
        assert_eq!(
            bundled_library_file_name("fixture", "win32").expect("windows filename"),
            "fixture.dll"
        );
    }
}
