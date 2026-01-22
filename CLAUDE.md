# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

See [CONTRIBUTING.md](CONTRIBUTING.md) for build commands, testing, code style, and development workflow.

## Workspace Structure

- **ldk-server** - Main daemon server (entry point: `src/main.rs`)
- **ldk-server-cli** - CLI client using clap
- **ldk-server-client** - Reqwest-based client library
- **ldk-server-protos** - Protocol buffer definitions and generated Rust code

## Development Rules

- Always ensure tests pass and lints are fixed before committing
- Run `cargo fmt --all` after every code change
- Never add new dependencies unless explicitly requested
- Please always disclose the use of any AI tools in commit messages and PR descriptions
