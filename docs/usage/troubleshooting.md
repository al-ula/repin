# Troubleshooting

## Workspace not initialized

Commands that need graph state report `PROJECT_NOT_INITIALIZED` when no `.repin/graph.sqlite3` is found. Run:

```bash
repin init
```

From another directory, select the repository explicitly with `repin --project /path/to/repository <COMMAND>`.

## Results are stale

Apply worktree changes first:

```bash
repin sync
repin status
```

If a derived view is damaged or incomplete, rebuild it. `all` rebuilds every supported view:

```bash
repin rebuild graph
repin rebuild all
```

## Configuration errors

Check the active file and merged values:

```bash
repin config validate
repin config show
```

If the file is elsewhere, pass `--config /path/to/config.toml`. Put provider profiles and credential environment-variable names in `~/.config/repin/config.toml`, not project configuration.

## Daemon connection failures

Inspect and restart the daemon:

```bash
repin daemon status
repin daemon restart
```

If the client and daemon report incompatible protocol versions, install matching Repin binaries and restart the daemon. `repin version --json` prints the compatibility fields needed to diagnose a mismatch.

For a custom runtime directory, pass `--runtime-dir <PATH>` to the daemon commands. Run `repin daemon run` in the foreground when collecting process or startup diagnostics.

## Database inspection

Inspect SQLite identity and schema without activating the graph:

```bash
repin db inspect
repin db inspect --json
```

Use `repin db migrate` only when an explicit schema migration is required.
