<a href="https://flux.sandydoo.me/">
  <img width="100%" src="https://github.com/sandydoo/gif-storage/blob/main/flux/social-header-2022-02-03.gif" alt="Flux" />
</a>

# Screensavers for Flux

I’m working on creating native screensavers for [Flux][flux] — a fluid simulation inspired by the MacOS Drift screensaver.

This repository contains:

- `flux-ffi` — a foreign function interface for the [Flux library][flux].
- Native screensavers for the following platforms:
  - [MacOS](#macos)
  - ~Windows~
  - ~Linux~

## Build for platform

### MacOS

First build the FFI crate:

```sh
export MACOSX_DEPLOYMENT_TARGET=10.10
cargo build --release --target x86_64-apple-darwin
```

Then compile with XCode.


[flux]: https://github.com/sandydoo/flux
