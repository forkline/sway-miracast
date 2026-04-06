# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [unreleased]

### Fixed

- Correct changelog generation and add missing tags ([e49abf3](https://github.com/forkline/swaybeam/commit/e49abf3ef6b6c11bed814598e650de9df63b8cf4))

### Documentation

- Update CHANGELOG with complete v0.4.0 history ([fa45de3](https://github.com/forkline/swaybeam/commit/fa45de31600b8121520b9c2b5b6ebb5ad08cf1f8))
- Update release skill with correct changelog generation ([1f67cad](https://github.com/forkline/swaybeam/commit/1f67cad775c92cc41ba26c0d1afdd4f6a8726a9a))
- Regenerate CHANGELOG with all version sections ([94049b2](https://github.com/forkline/swaybeam/commit/94049b26542a9b718a6b36deb0149133f310d8b8))
- Clean up CHANGELOG - keep only released versions ([33c95b8](https://github.com/forkline/swaybeam/commit/33c95b86a2b4ccc31bff54fa44aa537f663e95f8))
## [v0.4.0](https://github.com/forkline/swaybeam/tree/v0.4.0) - 2026-04-06

### Added

- external: Add virtual monitor support for Miracast streaming ([38a39b1](https://github.com/forkline/swaybeam/commit/38a39b1018a9abc4a150f6a0bc7d9a5d596ac40e))
## [v0.3.0](https://github.com/forkline/swaybeam/tree/v0.3.0) - 2026-04-06

### Added

- audio: Add virtual audio sink for TV streaming with --audio flag ([4ed0c15](https://github.com/forkline/swaybeam/commit/4ed0c15eaf4f5cf15f849140d1cae959b253802b))
- stream: Add audio capture support with optional --no-audio flag ([1504a0f](https://github.com/forkline/swaybeam/commit/1504a0fd85e0c6a2ac0bbee9c780c406575aa004))
- Add codec selection with H.264/H.265 hardware encoding support ([d38c617](https://github.com/forkline/swaybeam/commit/d38c6170801e960cb547ba81485f1aba82514b74))

### Fixed

- audio: Add better error handling for virtual audio sink ([8da944b](https://github.com/forkline/swaybeam/commit/8da944bcff6bd6b39d7329697a50d2d3736a2e55))
- cli: Disable H.265 codec options pending HDCP implementation ([e320175](https://github.com/forkline/swaybeam/commit/e320175509dc3111826cf0a5e244ff46ce573169))
## [v0.2.0](https://github.com/forkline/swaybeam/tree/v0.2.0) - 2026-04-05

### Added

- cli: Add --sink option to daemon command ([205f2fc](https://github.com/forkline/swaybeam/commit/205f2fca20172b8b3879dbd86736a538b6b0c57c))
- hdcp: Implement locality check phase with Kd derivation ([14d01b4](https://github.com/forkline/swaybeam/commit/14d01b4e251376dcfff3966b065f7a6674dbb60c))
- hdcp: Implement HDCP 2.2/2.3 IV construction with r_rx ([fb62606](https://github.com/forkline/swaybeam/commit/fb626064c47ead8a9665705a4f65a64bbd8a08f8))
- stream: Switch pipeline to MPEG-TS over RTP for Miracast compatibility ([626ff24](https://github.com/forkline/swaybeam/commit/626ff24b11684e4d98cb2fe13ddfb9269e3e94fe))
- Add test pattern streaming and IP discovery ([a52274e](https://github.com/forkline/swaybeam/commit/a52274e0ef7226be85667137d82b6597e328bfd4))
- Add RTSP debug server and test script ([e4853d5](https://github.com/forkline/swaybeam/commit/e4853d52e5ec195092782ff132eb98a87a8e48eb))
- Add RTSP client mode and WFD port parsing ([2c3b933](https://github.com/forkline/swaybeam/commit/2c3b9333293b76a8a3716fdc84e5fa3b922b78de))
- Add Wireshark/tcpdump capture scripts and complete HDCP AKE_No_Stored_km ([a14994b](https://github.com/forkline/swaybeam/commit/a14994b9e45d610a6d1dd561e4c9c2960133cb55))
- Add protocol-debug skill for packet capture debugging ([3c8e5da](https://github.com/forkline/swaybeam/commit/3c8e5da4bbc1679888c8868e46c112bb8c8670df))
- Replace test pattern with real PipeWire screen capture ([578a874](https://github.com/forkline/swaybeam/commit/578a8747f4ab4c2e5badb9a84a36ae9c30bd9a05))

### Fixed

- capture: Keep D-Bus connection alive to maintain portal session ([c5a4f4a](https://github.com/forkline/swaybeam/commit/c5a4f4a18df7ae15c8955c09b2df2749bcf5aa4b))
- daemon: Start RTSP server and keep streaming active ([a2ca5f7](https://github.com/forkline/swaybeam/commit/a2ca5f7adf091e05e1954ef2a682579c2fd6e606))
- hdcp: Send AKE_Transmitter_Info after cert, use version=3 ([85a0ec4](https://github.com/forkline/swaybeam/commit/85a0ec4f07d2e2a05fe3adab0d9db90404109d29))
- net: Correct NetworkManager WiFi P2P integration ([eb9b83e](https://github.com/forkline/swaybeam/commit/eb9b83e2f7acb8f0ff1a9c3d3af96be913207fe8))
- net: Use proper WFD source IEs ([6b94a81](https://github.com/forkline/swaybeam/commit/6b94a81d2864833507a847c78d33ac0d91da3b37))
- net: Correct WFD IEs for Miracast Source device ([2e6b27b](https://github.com/forkline/swaybeam/commit/2e6b27bd3bec8919a80aa620a8d6eb7385a41927))
- net: Wait for usable P2P readiness ([6a92f1c](https://github.com/forkline/swaybeam/commit/6a92f1c1bd1cfcedcc639d34ffdfb110104acdb1))
- net: Use transient NM P2P activation ([b020daa](https://github.com/forkline/swaybeam/commit/b020daa4b9d2f1eba66f0e1355dc1a0fe59e8b92))
- net: Use peer-bound P2P activation ([f171bad](https://github.com/forkline/swaybeam/commit/f171bad00089ca2de91f0d1bbd312f3867251c0e))
- rtsp: Wait for PLAY command before streaming ([33de16a](https://github.com/forkline/swaybeam/commit/33de16a7caac9bf443c6c50f2b4fd1549359047b))
- rtsp: Negotiate correctly with GO sinks ([f533b17](https://github.com/forkline/swaybeam/commit/f533b17313777e7cc67e72e5716465e5829bca75))
- rtsp: Handle LG reverse control connections ([f17d59c](https://github.com/forkline/swaybeam/commit/f17d59c6332b9e603a370e7e0072f78afe69e40a))
- rtsp: Use sink RTP ports in reverse mode ([19f1345](https://github.com/forkline/swaybeam/commit/19f1345064e3a9f3713546b14e429737f27a808e))
- rtsp: Honor LG content protection setup ([18a8456](https://github.com/forkline/swaybeam/commit/18a845661239593c9190058bae9806fb5e66ec3a))
- rtsp: Start HDCP ake with LG sinks ([569655b](https://github.com/forkline/swaybeam/commit/569655bd50270645d36bcf6a097d30a8f905967a))
- rtsp: Use correct WFD trigger_method SETUP flow instead of direct PLAY ([9a59a69](https://github.com/forkline/swaybeam/commit/9a59a697ce2518a54a388ec1e881d23975a6bd8b))
- stream: Transfer PipeWireStream ownership to prevent early fd closure ([e3f96b0](https://github.com/forkline/swaybeam/commit/e3f96b01aed99835929fde067db80145df18e707))
- stream: Use programmatic pipewiresrc setup with path property ([a65c86b](https://github.com/forkline/swaybeam/commit/a65c86b3a03f0032eb39968ff79176ec743ac707))
- stream: Set autoconnect=false and keepalive-time, and reliability ([e147370](https://github.com/forkline/swaybeam/commit/e1473700d1f17b9fc91486d847563b35c6bf203c))
- stream: Fix keepalive-time type and improve logging ([18210b6](https://github.com/forkline/swaybeam/commit/18210b6321b00a438f9b5576bb4942b01975054b))
- Match Android/WFD spec for Miracast compatibility ([82de3a2](https://github.com/forkline/swaybeam/commit/82de3a2738eb06cd174125c1ce3cb32a32d14e10))
- Match gnome-network-displays WFD IE format ([ff51faf](https://github.com/forkline/swaybeam/commit/ff51fafca03a02bbaa90b2c88ece7a5d1144d485))
- Correct WFD IE Device Info field - was 2 bytes, should be 1 byte ([1a612ca](https://github.com/forkline/swaybeam/commit/1a612ca332920231d95e5ff7868f7da4ff672d13))

### Documentation

- Add test results and debugging guide ([12a9c90](https://github.com/forkline/swaybeam/commit/12a9c909fc6c27a386357488dbfb89f4b0d1cf1a))
- Document screen capture investigation findings ([915a025](https://github.com/forkline/swaybeam/commit/915a0257d5919ef12bf1b549eb6170b9e1d74348))
- Document the fix for pipewiresrc path property ([04c3e7e](https://github.com/forkline/swaybeam/commit/04c3e7e59077fa309d9a24569cd2cd9ed9363271))
- Update screen capture investigation findings ([6c43f93](https://github.com/forkline/swaybeam/commit/6c43f9318f37c26255a56118ba9a38d5deb0b823))

### Debug

- hdcp: Add extensive logging for H_prime mismatch investigation ([ef39f4a](https://github.com/forkline/swaybeam/commit/ef39f4a37c30d81254d8ce0ab1baaf9b5001270b))

### Styling

- Apply cargo fmt formatting ([4c80e4a](https://github.com/forkline/swaybeam/commit/4c80e4a7eb64b26a3fd6d6af0c568d1e2663e79f))
## [v0.1.2](https://github.com/forkline/swaybeam/tree/v0.1.2) - 2026-04-02

### Fixed

- ci: Use VERSION variable for git tag in PKGBUILD source ([2d35254](https://github.com/forkline/swaybeam/commit/2d352540c887c87ed9f1d42d9656f5ddf952d5bf))

### Chore

- Update release skill and scripts to match passless workflow ([73862fa](https://github.com/forkline/swaybeam/commit/73862fa03ad8e8979dcbdae46b4abc14b80d4698))
## [v0.1.1](https://github.com/forkline/swaybeam/tree/v0.1.1) - 2026-04-02

### Added

- Complete implementation of all core crates ([7f49153](https://github.com/forkline/swaybeam/commit/7f491535a4b29bb14c5d2a9191948f6dcfc394a4))
- Integrate real system libraries (PipeWire, GStreamer, NetworkManager D-Bus) ([068873c](https://github.com/forkline/swaybeam/commit/068873cf059528a70c460df851c2af7a17e1be1f))
- Improve capture and doctor implementations ([ae6b4f5](https://github.com/forkline/swaybeam/commit/ae6b4f51f4aeb4df350158f697827a0f2b3a3790))
- Add H.265/AV1 codec support, 4K presets, RTSP shutdown, fix GStreamer bus guard ([153a683](https://github.com/forkline/swaybeam/commit/153a6833570ea23ea9070259b30cd272de93efff))
- Add WFD codec negotiation and integration tests for H.265/AV1 ([26d013c](https://github.com/forkline/swaybeam/commit/26d013c91e6375b9f183146ca16507d1d7558f7b))
- Improve doctor checks for better diagnostics ([8e89071](https://github.com/forkline/swaybeam/commit/8e890718014805d38be0cea3271c3596dc310036))
- Rename project to swaybeam ([0334d67](https://github.com/forkline/swaybeam/commit/0334d672d89b28676c962d8ec512e2037707098a))
- Add comprehensive justfile and simplify documentation ([45b6e28](https://github.com/forkline/swaybeam/commit/45b6e28e8658fa1ae5af478c8317cf30b7835c09))
- Add comprehensive realistic tests and mock Miracast sink ([432a40f](https://github.com/forkline/swaybeam/commit/432a40fa4cffd13eb55caadf6b7b02f018e06137))

### Fixed

- ci: Remove cross-compilation targets, only support x86_64 Linux ([eaea4f5](https://github.com/forkline/swaybeam/commit/eaea4f5ca14f33b33a84bc8fc4a3f5fbaf011c6b))
- ci: Use bash script instead of Rash for PKGBUILD generation ([cf31f0e](https://github.com/forkline/swaybeam/commit/cf31f0e0b877043be8b1d007e8233549404264a4))
- Remove trailing whitespace ([3d04bb7](https://github.com/forkline/swaybeam/commit/3d04bb7b2881f82522f90013fe7d080f3f44579b))
- Fix integration tests and clean up unused test files ([b7c9102](https://github.com/forkline/swaybeam/commit/b7c9102d519a0f872b6ed8b128aff47a8ec169d0))
- Critical maintainability improvements ([67a8038](https://github.com/forkline/swaybeam/commit/67a8038451286aabbe5c90765bfcf1243548407a))
- Add test-validation and mock-sink commands to justfile ([e31f3b9](https://github.com/forkline/swaybeam/commit/e31f3b93db5985fbd9b056d78302ffff52ff95d7))
- Resolve all clippy warnings in validation tools and test files ([79905b6](https://github.com/forkline/swaybeam/commit/79905b67adab9b9f81de2fc6930cf1d61d13152f))
- Add system dependencies for GStreamer and PipeWire in CI ([b4eddb3](https://github.com/forkline/swaybeam/commit/b4eddb3e23ee05f9265aa0aad23642c3fbb71c87))
- Final clippy fixes for CI ([7d751d4](https://github.com/forkline/swaybeam/commit/7d751d43f4205d0b0fc762eb2410c1ed0ac4984a))
- Handle Internal error in GStreamer test for CI ([166b25f](https://github.com/forkline/swaybeam/commit/166b25fed41d3444bf37b60d378f0483b9ed25d6))
- Handle GStreamer plugin unavailability in codec tests ([aaef697](https://github.com/forkline/swaybeam/commit/aaef69757cf1ab12a0e394ab7c4377eb10729608))

### Documentation

- Update README with CI/CD and development commands ([cc98993](https://github.com/forkline/swaybeam/commit/cc989938b625377590328301ae0c1345a1cda285))
- Refresh README for Arch-first setup ([9d0827b](https://github.com/forkline/swaybeam/commit/9d0827b4a517ab2d6e29b39701ed30372a590fdc))
- Simplify and improve documentation for easy installation ([96f08c7](https://github.com/forkline/swaybeam/commit/96f08c7dc030db6dd1ad2ce25e6d87f1ca784327))

### Chore

- Add renovate config for dependency updates ([a46e713](https://github.com/forkline/swaybeam/commit/a46e7133ddb84947673a9bc9679e102aa75bae4f))

### Ci

- Add complete CI/CD infrastructure, AUR packaging, and release automation ([8392b32](https://github.com/forkline/swaybeam/commit/8392b328669e2ebfce44dda12f8e00b60ffb5167))
