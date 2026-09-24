"""Launch the desktop application or an Android emulator build."""

import argparse
import os
import subprocess
import sys
import time
from pathlib import Path

from . import android

REPO = Path(__file__).resolve().parent.parent


def adb_value(adb: str | Path, serial: str, key: str, env: dict[str, str]) -> str:
    shell_env = {**env, "MSYS_NO_PATHCONV": "1"}
    return subprocess.check_output(
        [str(adb), "-s", serial, "shell", "getprop", key], env=shell_env, text=True
    ).strip()


def matching_emulator(adb: str | Path, abi: str, env: dict[str, str]) -> str | None:
    devices = subprocess.check_output([str(adb), "devices"], env=env, text=True)
    for line in devices.splitlines():
        fields = line.strip().split()
        if (
            len(fields) >= 2
            and fields[0].startswith("emulator-")
            and fields[0][9:].isdigit()
            and fields[1] == "device"
        ) and adb_value(adb, fields[0], "ro.product.cpu.abi", env) == abi:
            return fields[0]
    return None


def ini_value(path: Path, key: str) -> str | None:
    if not path.is_file():
        return None
    return android.parse_java_properties(path.read_text()).get(key)


def avd_abi(home: Path, name: str) -> str | None:
    path = ini_value(home / f"{name}.ini", "path")
    return (
        ini_value(android.native_path(path) / "config.ini", "abi.type")
        if path
        else None
    )


def run_android(profile: str, arch: str) -> None:
    abi = android.abi_for_arch(arch)
    sdk = android.native_path(
        os.environ.get("ANDROID_HOME")
        or os.environ.get("ANDROID_SDK_ROOT")
        or str(Path(os.environ.get("LOCALAPPDATA", "")) / "Android/Sdk")
    )
    suffix = ".exe" if sys.platform == "win32" else ""
    adb = sdk / "platform-tools" / f"adb{suffix}"
    emulator = sdk / "emulator" / f"emulator{suffix}"
    apk = REPO / (
        "android/app/build/outputs/apk/dev/debug/app-dev-debug.apk"
        if profile == "dev"
        else "android/app/build/outputs/apk/prod/release/app-prod-release.apk"
    )
    application_id = "dbg.lapiz.dev" if profile == "dev" else "app.lapiz.dev"
    env = {**os.environ, "ANDROID_HOME": str(sdk)}
    if not apk.is_file():
        raise RuntimeError(
            f"Android APK not found; run 'just build android {profile} {arch}' first"
        )

    serial = matching_emulator(adb, abi, env)
    if not serial:
        avd_home = android.native_path(
            os.environ.get("ANDROID_AVD_HOME")
            or str(
                android.native_path(
                    os.environ.get("ANDROID_USER_HOME", str(Path.home() / ".android"))
                )
                / "avd"
            )
        )
        avd = os.environ.get("ANDROID_AVD")
        if not avd:
            for candidate in subprocess.check_output(
                [str(emulator), "-list-avds"], env=env, text=True
            ).splitlines():
                if avd_abi(avd_home, candidate.strip()) == abi:
                    avd = candidate.strip()
                    break
        if not avd or avd_abi(avd_home, avd) != abi:
            raise RuntimeError(
                f"No native AVD for {abi} found; use an AVD with this ABI or run the x86_64 build on an x86_64 AVD"
            )
        subprocess.Popen(
            [
                str(emulator),
                "-avd",
                avd,
                "-gpu",
                os.environ.get("ANDROID_EMULATOR_GPU", "host"),
                "-no-snapshot-load",
            ],
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        for _ in range(120):
            serial = matching_emulator(adb, abi, env)
            if serial:
                break
            time.sleep(2)
        if not serial:
            raise RuntimeError(f"Emulator for {abi} did not become available")

    for _ in range(120):
        if adb_value(adb, serial, "sys.boot_completed", env) == "1":
            break
        time.sleep(2)
    else:
        raise RuntimeError(f"Emulator {serial} did not finish booting")

    subprocess.run(
        [str(adb), "-s", serial, "install", "-r", str(apk)], env=env, check=True
    )
    subprocess.run([str(adb), "-s", serial, "logcat", "-c"], env=env, check=True)
    subprocess.run(
        [
            str(adb),
            "-s",
            serial,
            "shell",
            "am",
            "start",
            "-n",
            f"{application_id}/app.lapiz.dev.LapizActivity",
        ],
        env={**env, "MSYS_NO_PATHCONV": "1"},
        check=True,
    )
    subprocess.run(
        [
            str(adb),
            "-s",
            serial,
            "logcat",
            "-v",
            "time",
            "-s",
            "RustStdoutStderr:V",
            "AndroidRuntime:E",
            "libc:F",
        ],
        env=env,
        check=True,
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("platform", choices=("desktop", "android"))
    parser.add_argument("profile", choices=("dev", "dev-local", "release"))
    args = parser.parse_args()

    if args.platform == "desktop":
        command = ["cargo", "run"]
        if args.profile == "release":
            command.append("--release")
        command.append("--locked")
        if args.profile == "dev-local":
            command.extend(("--features", "lapiz_dirs/dev_local"))
        subprocess.run(command, cwd=REPO, check=True)
    elif args.platform == "android":
        if args.profile == "dev-local":
            parser.error("Android profile must be dev or release")
        if not android.HOST_ARCH:
            parser.error("unknown architecture")
        run_android(args.profile, android.HOST_ARCH)
    else:
        parser.error("platform must be desktop or android")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
