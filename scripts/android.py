"""Pinned Android build tools and environment for Gradle."""

import hashlib
import os
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import zipfile
from collections.abc import Mapping
from pathlib import Path


def parse_java_properties(props: str) -> dict[str, str]:
    values: dict[str, str] = {}
    for line in props.splitlines():
        line = line.strip()
        if not line or line.startswith(("#", "!")) or "=" not in line:
            continue
        key, value = line.split("=", 1)
        values[key.strip()] = value.strip()
    return values


REPO = Path(__file__).resolve().parent.parent
ROOT = REPO / "target/android-toolchain"
SDK = ROOT / "sdk"
TOOLCHAIN: dict[str, str] = parse_java_properties(
    (REPO / "android/toolchain.properties").read_text()
)
HOST = (
    "windows"
    if sys.platform == "win32"
    else "mac"
    if sys.platform == "darwin"
    else "linux"
)
HOST_ARCH: str | None = {
    "x86_64": "x86_64",
    "AMD64": "x86_64",
    "aarch64": "aarch64",
    "arm64": "aarch64",
}.get(platform.machine())
JDK_VERSION = TOOLCHAIN["ANDROID_JDK_VERSION"]
JDK_RELEASE = f"jdk-{JDK_VERSION}"
JDK = ROOT / JDK_RELEASE
JAVA_HOME = JDK / "Contents/Home" if HOST == "mac" else JDK
EXE = ".exe" if HOST == "windows" else ""


def run(
    *command: str | Path,
    cwd: str | Path | None = None,
    env: Mapping[str, str] | None = None,
    input: str | None = None,
    text: bool = False,
) -> None:
    subprocess.run(
        [str(part) for part in command],
        check=True,
        cwd=cwd,
        env=env,
        input=input,
        text=text,
    )


def assert_revision(path: Path, expected: str) -> None:
    if not path.is_file():
        raise RuntimeError(f"Missing Android package metadata: {path}")
    actual = parse_java_properties(path.read_text()).get("Pkg.Revision", "")
    if actual != expected:
        raise RuntimeError(
            f"Unexpected Android package revision in {path}: {actual} (expected {expected})"
        )


def assert_sdk_revisions() -> None:
    assert_revision(
        SDK / f"platforms/android-{TOOLCHAIN['ANDROID_PLATFORM']}/source.properties",
        TOOLCHAIN["ANDROID_PLATFORM_REVISION"],
    )
    assert_revision(
        SDK / f"build-tools/{TOOLCHAIN['ANDROID_BUILD_TOOLS']}/source.properties",
        TOOLCHAIN["ANDROID_BUILD_TOOLS"],
    )
    assert_revision(
        SDK / f"ndk/{TOOLCHAIN['ANDROID_NDK']}/source.properties",
        TOOLCHAIN["ANDROID_NDK"],
    )


def jdk_ready() -> bool:
    release = JAVA_HOME / "release"
    return (
        (JAVA_HOME / f"bin/java{EXE}").is_file()
        and release.is_file()
        and f'JAVA_RUNTIME_VERSION="{JDK_VERSION}"' in release.read_text().splitlines()
    )


def cargo_ndk_ready() -> bool:
    binary = ROOT / f"cargo/bin/cargo-ndk{EXE}"
    if not binary.is_file():
        return False
    env = os.environ.copy()
    env["PATH"] = os.pathsep.join((str(ROOT / "cargo/bin"), env.get("PATH", "")))
    result = subprocess.run(
        ["cargo", "ndk", "--version"],
        capture_output=True,
        text=True,
        check=False,
        env=env,
    )
    return (
        result.returncode == 0
        and result.stdout.strip() == f"cargo-ndk {TOOLCHAIN['ANDROID_CARGO_NDK']}"
    )


def env_vars() -> dict[str, str]:
    if not jdk_ready() or not cargo_ndk_ready():
        raise RuntimeError(
            "Android toolchain missing or mismatched; run 'just setup-android'"
        )
    assert_sdk_revisions()
    assert_revision(
        SDK
        / f"cmdline-tools/{TOOLCHAIN['ANDROID_CMDLINE_TOOLS_VERSION']}/source.properties",
        TOOLCHAIN["ANDROID_CMDLINE_TOOLS_VERSION"],
    )

    env = os.environ.copy()
    env.update(
        JAVA_HOME=str(JAVA_HOME),
        ANDROID_HOME=str(SDK),
        ANDROID_SDK_ROOT=str(SDK),
        ANDROID_NDK_HOME=str(SDK / "ndk" / TOOLCHAIN["ANDROID_NDK"]),
    )
    env["PATH"] = os.pathsep.join(
        (str(ROOT / "cargo/bin"), str(JAVA_HOME / "bin"), env.get("PATH", ""))
    )
    return env


def native_path(path: str | Path) -> Path:
    if HOST == "windows" and str(path).startswith("/"):
        return Path(
            subprocess.check_output(["cygpath", "-w", str(path)], text=True).strip()
        )
    return Path(path)


def abi_for_arch(arch: str) -> str:
    if arch not in ("aarch64", "x86_64"):
        raise ValueError(
            f"Unsupported Android architecture: {arch} (choose aarch64 or x86_64)"
        )
    return TOOLCHAIN[f"ANDROID_ABI_{arch.upper()}"]


def rust_target_for_arch(arch: str) -> str:
    abi_for_arch(arch)
    return f"{arch}-linux-android"


def download(url: str, destination: Path) -> None:
    with urllib.request.urlopen(url) as response, destination.open("wb") as output:
        shutil.copyfileobj(response, output)


def install_jdk() -> None:
    if jdk_ready():
        return
    if JDK.exists():
        raise RuntimeError(
            f"Incomplete or mismatched JDK at {JDK}; remove it and rerun setup"
        )

    major = JDK_VERSION.split(".")[0]
    name = JDK_VERSION.replace("+", "_")
    extension = ".zip" if HOST == "windows" else ".tar.gz"
    filename = f"OpenJDK{major}U-jdk_{HOST_ARCH}_{'windows' if HOST == 'windows' else 'mac' if HOST == 'mac' else 'linux'}_hotspot_{name}{extension}"
    url = f"https://github.com/adoptium/temurin{major}-binaries/releases/download/{JDK_RELEASE.replace('+', '%2B')}/{filename}"

    ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".jdk.", dir=ROOT) as temporary:
        archive = Path(temporary) / filename
        checksum = Path(temporary) / "checksum"
        download(url, archive)
        download(url + ".sha256.txt", checksum)

        expected = checksum.read_text().split()[0]
        digest = hashlib.sha256()

        with archive.open("rb") as source:
            for chunk in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(chunk)
        actual = digest.hexdigest()
        if actual != expected:
            raise RuntimeError("JDK archive checksum mismatch")

        unpacked = Path(temporary) / "unpacked"
        unpacked.mkdir()
        if HOST == "windows":
            with zipfile.ZipFile(archive) as package:
                package.extractall(unpacked)
        else:
            with tarfile.open(archive, "r:gz") as package:
                package.extractall(unpacked)

        (unpacked / JDK_RELEASE).rename(JDK)


def install_commandline_tools() -> None:
    directory = SDK / "cmdline-tools" / TOOLCHAIN["ANDROID_CMDLINE_TOOLS_VERSION"]
    if not (directory / "source.properties").is_file():
        directory.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(
            prefix=".cmdline-tools.", dir=ROOT
        ) as temporary:
            filename = f"commandlinetools-{'win' if HOST == 'windows' else HOST}-{TOOLCHAIN['ANDROID_CMDLINE_TOOLS_ARCHIVE']}_latest.zip"
            archive = Path(temporary) / filename
            download(f"https://dl.google.com/android/repository/{filename}", archive)
            run(JAVA_HOME / f"bin/jar{EXE}", "xf", filename, cwd=temporary)
            (Path(temporary) / "cmdline-tools").rename(directory)
        if HOST != "windows":
            manager = directory / "bin/sdkmanager"
            manager.chmod(manager.stat().st_mode | 0o111)

    assert_revision(
        directory / "source.properties", TOOLCHAIN["ANDROID_CMDLINE_TOOLS_VERSION"]
    )


def install_sdk() -> None:
    packages = (
        SDK / f"platforms/android-{TOOLCHAIN['ANDROID_PLATFORM']}/source.properties",
        SDK / f"build-tools/{TOOLCHAIN['ANDROID_BUILD_TOOLS']}/source.properties",
        SDK / f"ndk/{TOOLCHAIN['ANDROID_NDK']}/source.properties",
    )
    if not all(path.is_file() for path in packages):
        manager = (
            SDK
            / "cmdline-tools"
            / TOOLCHAIN["ANDROID_CMDLINE_TOOLS_VERSION"]
            / "bin"
            / ("sdkmanager.bat" if HOST == "windows" else "sdkmanager")
        )
        env = os.environ.copy()
        env["JAVA_HOME"] = str(JAVA_HOME)
        license_input = "y\n" * 1000 if os.environ.get("CI") == "true" else None
        run(
            manager,
            f"--sdk_root={SDK}",
            "--licenses",
            env=env,
            input=license_input,
            text=True,
        )
        run(
            manager,
            f"--sdk_root={SDK}",
            "--install",
            f"platforms;android-{TOOLCHAIN['ANDROID_PLATFORM']}",
            f"build-tools;{TOOLCHAIN['ANDROID_BUILD_TOOLS']}",
            f"ndk;{TOOLCHAIN['ANDROID_NDK']}",
            env=env,
        )
    assert_sdk_revisions()


def install_cargo_ndk() -> None:
    if not cargo_ndk_ready():
        env = os.environ.copy()
        env["RUSTFLAGS"] = ""
        run(
            "cargo",
            "install",
            "cargo-ndk",
            "--locked",
            "--version",
            TOOLCHAIN["ANDROID_CARGO_NDK"],
            "--root",
            ROOT / "cargo",
            "--force",
            env=env,
        )

    run(
        "rustup",
        "target",
        "add",
        rust_target_for_arch("aarch64"),
        rust_target_for_arch("x86_64"),
    )


def main() -> None:
    if HOST not in ("windows", "mac", "linux") or HOST_ARCH is None:
        raise RuntimeError(
            f"Unsupported Android build host/architecture: {sys.platform}/{platform.machine()}"
        )
    install_jdk()
    install_commandline_tools()
    install_sdk()
    install_cargo_ndk()


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
