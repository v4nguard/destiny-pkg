# ⚠ WARNING: This library has been superseded by [tiger-pkg](https://github.com/v4nguard/tiger-pkg)

# Destiny PKG Library

[![Latest version](https://img.shields.io/crates/v/destiny-pkg.svg)](https://crates.io/crates/destiny-pkg)
[![Documentation](https://docs.rs/destiny-pkg/badge.svg)](https://docs.rs/destiny-pkg)
![Discord](https://img.shields.io/discord/948590455715684393?label=v4nguard%20discord&color=%2377aaff)

You need an oo2core DLL to be able to decompress packages.
When using destiny-pkg with a Destiny 2 installation, PackageManager will automatically search for oo2core
under `Destiny 2\bin\x64`.

In any other case, you will need to get oo2core_3_win64.dll from somewhere (an old game for example), and place it in
the
directory where you run destinypkgtool from. Check below for the version of oo2core that is required for your game.

## Package format support

| Version                         | Platform          | Works? | Oodle DLL |
|---------------------------------|-------------------|--------|-----------|
| Destiny Internal Alpha          | X360              | ✅      | oo2core_3 |
| Destiny Legacy (The Taken King) | PS3/X360/PS4/XONE | ✅      | oo2core_3 |
| Destiny (Rise of Iron)          | PS4/XONE          | ✅      | oo2core_3 |
| Destiny 2 (Beta)                | Any               | ✅      | oo2core_3 |
| Destiny 2 (Pre-BL)              | Any               | ✅      | oo2core_3 |
| Destiny 2 (Post-BL)             | Any               | ✅      | oo2core_9 |

## Game version codes

| Version               | Code          |
|-----------------------|---------------|
| DestinyInternalAlpha  | `d1_devalpha` |
| DestinyTheTakenKing   | `d1_ttk`      |
| DestinyRiseOfIron     | `d1_roi`      |
| Destiny2Beta          | `d2_beta`     |
| Destiny2Forsaken      | `d2_fs`       |
| Destiny2Shadowkeep    | `d2_sk`       |
| Destiny2BeyondLight   | `d2_bl`       |
| Destiny2WitchQueen    | `d2_wq`       |
| Destiny2Lightfall     | `d2_lf`       |
| Destiny2TheFinalShape | `d2_tfs`      |

## Platform codes

| Platform      | Code      |
|---------------|-----------|
| Xbox 360      | `x360`    |
| Xbox One      | `xboxone` |
| PlayStation 3 | `ps3`     |
| PlayStation 4 | `ps4`     |
| Windows (x64) | `w64`     |
