# Release

Before creating a release, check that everything can be published to crates.io.

1. If the C SDK changed:
  - Update the [checksums.txt](aic-sdk-sys/checksums.txt) with the one distributed in the C SDK
  - Update the [aic.h](aic-sdk-sys/include/aic.h) header file with the one distributed in the C SDK

2. If there were changes in `aic-sdk-sys`:
  - Increase version number in [aic-sdk-sys/Cargo.toml](aic-sdk-sys/Cargo.toml)
  - Set `aic-sdk-sys` dependency version number in top-level [Cargo.toml](Cargo.toml) to the newest version

3. Update the version number of the workspace in the top level [Cargo.toml](Cargo.toml)

4. Check that the right unsupported Rust version number is reflected in the [README.md](README.md) warning

5. Update [changelog](CHANGELOG.md)

5. Create a new release on the GitHub main branch with a tag that has the same version number as the main crate
