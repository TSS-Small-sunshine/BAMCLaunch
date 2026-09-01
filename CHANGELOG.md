# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Added
- CI workflow for frontend build + Rust check/test/clippy
- ESLint + Prettier for frontend
- rustfmt configuration
- CHANGELOG.md (this file)

## [0.1.0] - 2026-08-18
### Added
- Initial release: Tauri 2 + React 19 + TypeScript + Vite scaffold
- Minecraft version manifest fetching
- Version JSON / JAR / assets / libraries download (M2 partial)
- Java runtime detection (scan registry + PATH + JAVA_HOME)
- Game launch with UUID-based offline auth placeholder
- Settings persistence (game dir, Java override, memory)
- Running instances tracking
- BA×MC themed Chakra UI v2 UI
