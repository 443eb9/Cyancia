#!/usr/bin/env bash
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$repo/android/toolchain.properties"
root="$repo/target/android-toolchain"
sdk="$root/sdk"

case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) host=windows; ext=.exe; sdkmanager_ext=.bat ;;
    Darwin*) host=mac; ext=; sdkmanager_ext= ;;
    Linux*) host=linux; ext=; sdkmanager_ext= ;;
    *) echo "Unsupported Android build host: $(uname -s)" >&2; exit 1 ;;
esac

case "$(uname -m)" in
    x86_64|amd64) arch=x64 ;;
    aarch64|arm64) arch=aarch64 ;;
    *) echo "Unsupported Android build architecture: $(uname -m)" >&2; exit 1 ;;
esac

if [ "$host" = windows ] && [ "$arch" != x64 ]; then
    echo "Windows ARM64 is not supported by this toolchain" >&2
    exit 1
fi

jdk_major="${ANDROID_JDK_VERSION%%.*}"
jdk_release="jdk-$ANDROID_JDK_VERSION"
jdk="$root/$jdk_release"
if [ "$host" = mac ]; then
    java_home="$jdk/Contents/Home"
else
    java_home="$jdk"
fi

native_path() {
    if [ "$host" = windows ]; then
        cygpath -w "$1"
    else
        printf '%s\n' "$1"
    fi
}

revision() {
    local file="$1" key="$2"
    if [ ! -f "$file" ]; then
        echo "Missing Android package metadata: $file" >&2
        return 1
    fi
    awk -F= -v key="$key" '{
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", $1)
        if ($1 == key) {
            gsub(/^[[:space:]]+|[[:space:]]+$/, "", $2)
            print $2
            exit
        }
    }' "$file"
}

require_revision() {
    local actual
    actual="$(revision "$1" Pkg.Revision)"
    if [ "$actual" != "$2" ]; then
        echo "Unexpected Android package revision in $1: $actual (expected $2)" >&2
        return 1
    fi
}

jdk_ready() {
    [ -x "$java_home/bin/java$ext" ] &&
        [ -f "$java_home/release" ] &&
        grep -Fqx "JAVA_RUNTIME_VERSION=\"$ANDROID_JDK_VERSION\"" "$java_home/release"
}

install_jdk() {
    if jdk_ready; then return; fi
    if [ -e "$jdk" ]; then
        echo "Incomplete or mismatched JDK at $jdk; remove it and rerun setup" >&2
        return 1
    fi
    local name file url temporary actual expected
    name="${ANDROID_JDK_VERSION/+/_}"
    case "$host/$arch" in
        windows/x64) file="OpenJDK${jdk_major}U-jdk_x64_windows_hotspot_${name}.zip" ;;
        linux/x64) file="OpenJDK${jdk_major}U-jdk_x64_linux_hotspot_${name}.tar.gz" ;;
        linux/aarch64) file="OpenJDK${jdk_major}U-jdk_aarch64_linux_hotspot_${name}.tar.gz" ;;
        mac/x64) file="OpenJDK${jdk_major}U-jdk_x64_mac_hotspot_${name}.tar.gz" ;;
        mac/aarch64) file="OpenJDK${jdk_major}U-jdk_aarch64_mac_hotspot_${name}.tar.gz" ;;
    esac
    url="https://github.com/adoptium/temurin${jdk_major}-binaries/releases/download/${jdk_release/+/%2B}/$file"
    mkdir -p "$root"
    temporary="$(mktemp -d "$root/.jdk.XXXXXX")"
    curl -fL "$url" -o "$temporary/$file"
    curl -fL "$url.sha256.txt" -o "$temporary/checksum"
    expected="$(awk '{print $1; exit}' "$temporary/checksum")"
    if command -v sha256sum >/dev/null; then
        actual="$(sha256sum "$temporary/$file" | awk '{print $1}')"
    else
        actual="$(shasum -a 256 "$temporary/$file" | awk '{print $1}')"
    fi
    if [ -z "$expected" ] || [ "$actual" != "$expected" ]; then
        echo "JDK archive checksum mismatch" >&2
        exit 1
    fi
    mkdir -p "$temporary/unpacked"
    if [ "$host" = windows ]; then
        /c/Windows/System32/tar.exe -xf "$temporary/$file" -C "$temporary/unpacked"
    else
        tar -xzf "$temporary/$file" -C "$temporary/unpacked"
    fi
    mv "$temporary/unpacked/$jdk_release" "$jdk"
    rm -rf "$temporary"
}

install_commandline_tools() {
    local directory="$sdk/cmdline-tools/$ANDROID_CMDLINE_TOOLS_VERSION"
    if [ ! -f "$directory/source.properties" ]; then
        local temporary file archive_host
        mkdir -p "$sdk/cmdline-tools"
        temporary="$(mktemp -d "$root/.cmdline-tools.XXXXXX")"
        archive_host="$host"
        if [ "$host" = windows ]; then archive_host=win; fi
        file="commandlinetools-$archive_host-$ANDROID_CMDLINE_TOOLS_ARCHIVE"_latest.zip
        curl -fL "https://dl.google.com/android/repository/$file" -o "$temporary/$file"
        (cd "$temporary" && "$java_home/bin/jar$ext" xf "$file")
        mv "$temporary/cmdline-tools" "$directory"
        rm -rf "$temporary"
        if [ "$host" != windows ]; then chmod +x "$directory/bin/sdkmanager"; fi
    fi
    require_revision "$directory/source.properties" "$ANDROID_CMDLINE_TOOLS_VERSION"
}

require_sdk_revisions() {
    require_revision "$sdk/platforms/android-$ANDROID_PLATFORM/source.properties" "$ANDROID_PLATFORM_REVISION"
    require_revision "$sdk/build-tools/$ANDROID_BUILD_TOOLS/source.properties" "$ANDROID_BUILD_TOOLS"
    require_revision "$sdk/ndk/$ANDROID_NDK/source.properties" "$ANDROID_NDK"
}

install_sdk() {
    if [ -f "$sdk/platforms/android-$ANDROID_PLATFORM/source.properties" ] &&
       [ -f "$sdk/build-tools/$ANDROID_BUILD_TOOLS/source.properties" ] &&
       [ -f "$sdk/ndk/$ANDROID_NDK/source.properties" ]; then
        require_sdk_revisions
        return
    fi

    local manager="$sdk/cmdline-tools/$ANDROID_CMDLINE_TOOLS_VERSION/bin/sdkmanager$sdkmanager_ext"
    local sdk_path
    sdk_path="$(native_path "$sdk")"
    export JAVA_HOME
    JAVA_HOME="$(native_path "$java_home")"
    if [ "${CI:-}" = true ]; then
        "$manager" --sdk_root="$sdk_path" --licenses < <(yes)
    else
        "$manager" --sdk_root="$sdk_path" --licenses
    fi
    "$manager" --sdk_root="$sdk_path" --install \
        "platforms;android-$ANDROID_PLATFORM" \
        "build-tools;$ANDROID_BUILD_TOOLS" \
        "ndk;$ANDROID_NDK"
    require_sdk_revisions
}

cargo_ndk_ready() {
    local binary="$root/cargo/bin/cargo-ndk$ext"
    [ -x "$binary" ] &&
        [ "$(PATH="$root/cargo/bin:$PATH" cargo ndk --version)" = "cargo-ndk $ANDROID_CARGO_NDK" ]
}

install_cargo_ndk() {
    if ! cargo_ndk_ready; then
        RUSTFLAGS= cargo install cargo-ndk --locked --version "$ANDROID_CARGO_NDK" --root "$root/cargo" --force
    fi
    rustup target add "$(rust_target_for_arch aarch64)" "$(rust_target_for_arch x86_64)"
}

android_abi_for_arch() {
    case "$1" in
        aarch64) printf '%s\n' "$ANDROID_ABI_AARCH64" ;;
        x86_64) printf '%s\n' "$ANDROID_ABI_X86_64" ;;
        *) echo "Unsupported Android architecture: $1 (choose aarch64 or x86_64)" >&2; return 2 ;;
    esac
}

rust_target_for_arch() {
    android_abi_for_arch "$1" > /dev/null || return
    printf '%s-linux-android\n' "$1"
}

use_toolchain() {
    if ! jdk_ready || ! cargo_ndk_ready; then
        echo "Android toolchain missing or mismatched; run 'just setup-android'" >&2
        return 1
    fi
    require_sdk_revisions
    require_revision "$sdk/cmdline-tools/$ANDROID_CMDLINE_TOOLS_VERSION/source.properties" "$ANDROID_CMDLINE_TOOLS_VERSION"
    export JAVA_HOME="$java_home"
    export ANDROID_HOME="$(native_path "$sdk")"
    export ANDROID_NDK_HOME="$(native_path "$sdk/ndk/$ANDROID_NDK")"
    export PATH="$root/cargo/bin:$java_home/bin:$PATH"
}

case "${1:-}" in
    setup)
        install_jdk
        install_commandline_tools
        install_sdk
        install_cargo_ndk
        ;;
    use) use_toolchain ;;
    '') ;;
    *) echo "Usage: source android/toolchain.sh; use_toolchain | bash android/toolchain.sh setup" >&2; exit 2 ;;
esac
