---
created: 2026-03-04T05:10:57.357Z
title: Create README docs for cargo-pmcp CLI
area: docs
files:
  - cargo-pmcp/README.md
  - cargo-pmcp/src/commands/
---

## Problem

The `cargo-pmcp` CLI crate lacks comprehensive README documentation. Users and contributors need clear documentation covering:
- What `cargo pmcp` does and how to install it
- Available subcommands (auth, deploy, init, loadtest, preview, validate, etc.)
- Usage examples for common workflows
- Configuration options and flags (e.g., --quiet, --json)

## Solution

Create or update `cargo-pmcp/README.md` with:
- Installation instructions (cargo install)
- Command reference table with descriptions
- Usage examples for key workflows (deploy, loadtest, preview)
- Global flags documentation (--quiet, --json)
- Links to further docs or the main pmcp README
