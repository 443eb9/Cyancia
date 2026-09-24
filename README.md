# Lapiz

_Project logo under construction_

> [!WARNING]
> This project is still at pre-pre-pre-alpha stage, and is absolutely not intended for production use. It has tons of bugs and incomplete code!

![](./docs/readme/main.png)

> Cute orange photo by Dariusz Duchiewicz on [Pexels](https://www.pexels.com/photo/bright-basket-of-oranges-and-apples-36492525/)

A GPU powered, programmable, highly customizable and blazing fast digital painting program written in Rust, build with ❤ and passion, and open-source forever under the GPL-3.0-or-later License.

The name "Lapiz" means pencil in Spanish. It looks like the English word "Lapis", which is a kind of blue, natural blue mineral and one of the oldest and most precious blue pigments. About the pronounciation, neither English nor Spanish is my native language, so it pronounces whatever you like.

## Development

Lapiz uses [just](https://just.systems/) for local builds. Install it alongside Rustup, Python 3, Bash (Git Bash on Windows), Node.js matching [`.node-version`](.node-version), npm, and pipx, then run `just setup`. It prepares the desktop tools and the pinned Android build toolchain; Android setup also needs a network connection and acceptance of the SDK licenses. Use `just setup-base` or `just setup-android` to prepare only one side.

```bash
just test  # unit, documentation, and WGSL tests
just check # formatting, Clippy, and repository lints
just fmt   # format code
```

### Desktop

```bash
just build desktop dev     # compile Lapiz
just run desktop dev       # build and launch
just run desktop dev-local # keep app data under target/ and use local assets
just package desktop dev   # build a distributable archive
```

Desktop commands also accept `release`; `dev-local` is available for `run` only. Packages, checksums, and debug symbols are written to `target/package/`.

### Android

Android toolchain versions are pinned in [`android/toolchain.properties`](android/toolchain.properties) and installed under `target/android-toolchain/`. Specify a Rust architecture for every command: `aarch64` (Android ABI `arm64-v8a`) or `x86_64` (Android ABI `x86_64`).

```bash
just build android dev aarch64       # APK for ARM64 devices
just package android release aarch64 # APK, native symbols, and checksums in target/package/
just run android dev                 # build, launch a matching emulator, and stream logs
```

Android commands require `dev` or `release` and the architecture. `run` needs Android Emulator and Platform Tools installed separately; it uses a connected emulator whose primary ABI matches the build or starts an installed matching AVD (`ANDROID_AVD` can name one). To use an x86_64 AVD, select `x86_64` when building and running. The release APK uses a **debug key for local testing only**.

## LLM Assisted Contributions

This project is accepting LLM assisted contributions. BUT will absolutely reject any code that is not **reviewed by human**.

## Special Thanks

- [Bevy](https://bevy.org/)
- [Blender](https://www.blender.org/)
- [Krita](https://krita.org/)
- [LINUX DO](https://linux.do/)
- [Zed](https://zed.dev/)

## License

This project is licensed under the GPL-3.0-or-later License. See [LICENSE](LICENSE) for details.
