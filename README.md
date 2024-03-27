# Destiny PKG Library

[![Latest version](https://img.shields.io/crates/v/destiny-pkg.svg)](https://crates.io/crates/destiny-pkg)
[![Documentation](https://docs.rs/destiny-pkg/badge.svg)](https://docs.rs/destiny-pkg)
![Discord](https://img.shields.io/discord/948590455715684393?label=v4nguard%20discord&color=%2377aaff)

You need an oo2core DLL to be able to decompress packages.
When using destiny-pkg with a Destiny 2 installation, the library will automatically search for oo2core
under `Destiny 2\bin\x64`.

In any other case, you will need to get oo2core_3_win64.dll from somewhere (an old game for example), and place it in
the
directory where you run destinypkgtool from. Check below for the version of oo2core that is required for your game.

## Version support

| Version                         | Platform | Works? | Oodle DLL |
|---------------------------------|----------|--------|-----------|
| Destiny Legacy (The Taken King) | PS3/X360 | ✅      | oo2core_3 |
| Destiny Legacy (The Taken King) | PS4/XONE | ✅      | oo2core_3 |
| Destiny (Rise of Iron)          | PS4/XONE | ✅      | oo2core_3 |
| Destiny 2 (Beta)                | PC       | ✅      | oo2core_3 |
| Destiny 2 (Pre-BL)              | PC       | ✅      | oo2core_3 |
| Destiny 2 (Beyond Light)        | PC       | ✅      | oo2core_9 |
| Destiny 2 (Witch Queen)         | PC       | ✅      | oo2core_9 |
| Destiny 2 (Lightfall)           | PC       | ✅      | oo2core_9 |
