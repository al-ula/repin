# Usage Guide

This guide explains how to build, configure, and use the `repin` command-line client.

Architecture, contracts, and design decisions live in [Architecture & Specification](../code/index.html).

## Start here

- [Quick start](quickstart.md) — build Repin, initialize a repository, index it, and run the first searches.
- [CLI reference](cli.md) — commands grouped by task with practical examples.
- [Configuration](configuration.md) — project and global configuration, defaults, and optional model providers.
- [Agent integration](integration.md) — context generation, review workflows, and rerank callbacks.
- [Troubleshooting](troubleshooting.md) — recovery steps for initialization, indexing, configuration, and daemon failures.

The CLI accepts `--project <PATH>` to select a repository and `--config <PATH>` to select an explicit configuration file. Every command also exposes focused help through `repin <command> --help`.
