use anyhow::{Result, bail};

pub(crate) fn current_host_prebuild_target() -> Result<String> {
    let platform = current_node_platform()?;
    let arch = current_node_arch()?;

    if platform != "linux" {
        return Ok(format!("{platform}-{arch}"));
    }

    Ok(format!("{platform}-{arch}-{}", current_linux_libc()?))
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
