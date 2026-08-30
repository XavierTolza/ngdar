# NGDAR — New Generation Disk Archiving

[![CI](https://github.com/XavierTolza/ngdar/actions/workflows/ci.yml/badge.svg)](https://github.com/XavierTolza/ngdar/actions/workflows/ci.yml)

**NGDAR** is a Git-like incremental archiving tool for massive binary files. It produces standard `.tar` archives suitable for long-term cold storage (DVD, LTO tape, cloud). Unlike traditional backup tools, NGDAR tracks file history through content-addressed metadata, making it possible to know exactly which volume a file was written to — even decades later, with nothing more than a plain text editor.

## Architecture

NGDAR is built on three principles:

1. **Content-Addressable Storage** — Every file is identified by its BLAKE3 hash. The hash is computed once and cached in `~/.cache/ngdar/<repo-id>/cache.tsv` for fast subsequent runs.
2. **Plain-text metadata** — All history (commits, trees, file metadata) is stored as readable text files named by their own BLAKE3 hash. No database, no proprietary format.
3. **Self-contained archives** — Each `.tar` archive includes the *complete* metadata history plus *only the binary files changed in this session*. Any single archive tells you everything about the project.

## Repository Structure

```
.ngdar/              # Local metadata (no binary duplication)
├── repository_id    # UUID v4 linking project to its OS cache
├── HEAD             # Pointer to the latest commit hash
├── index            # Staging area (files ready for next pack)
└── objects/         # Content-addressed object store
    ├── ab/          # First 2 chars of hash = directory
    │   └── cd...    # Meta, Tree, or Commit object
    └── ...
```

## Object Model (Plain Text)

All objects are human-readable text files:

### Meta Object
```
type meta
size 1048576
mtime 1788118000
permissions 644
binary_hash a1b2c3d4e5f6...
volume_id DVD-001
```

### Tree Object
```
tree e3b0c442... sub_dir
meta 81a44c7...  document.pdf
```

### Commit Object
```
tree <root-tree-hash>
parent <parent-commit-hash>
author Xavier <xavier@example.com> 1788118091
os Linux 6.6.0-x86_64
tool_version ngdar-1.0.0

Descriptive commit message.
```

## CLI Commands

### `ngdar init`
Initialize a new repository in the current directory.

```bash
cd /path/to/data/
ngdar init
```

### `ngdar add <paths>`
Stage files for the next archive. Files are hashed (BLAKE3) and registered in the staging index. Supports `.ngdarignore` files for exclusions.

```bash
ngdar add documents/ photos/
ngdar add config.json
```

### `ngdar status`
Show the repository state in three categories:
- **Staged** — ready to be packed (diff between HEAD and index)
- **Unstaged** — changed on disk but not staged (verified via OS cache)
- **Untracked** — files not yet tracked (respects `.ngdarignore`)

```bash
ngdar status
```

### `ngdar pack --vol-id <ID> --out <archive.tar> -m "<message>"`
Create a TAR archive from staged files. This command:
1. Creates **Meta** objects with the volume ID (e.g., `DVD-001`)
2. Builds **Tree** and **Commit** objects
3. Generates a `.tar` containing:
   - The **full `.ngdar/`** metadata directory (complete project history)
   - A **`data/`** folder with the actual binary files from this session
4. Clears the staging index

```bash
ngdar pack --vol-id "ARCHIVE-2026-08" --out session_001.tar -m "Added administrative records"
```

## Archive Format

The output `.tar` archive has two top-level directories:

```
.ngdar/              # Complete metadata history
├── repository_id
├── HEAD
├── index
└── objects/
    ├── ...
data/                # Incremental binary files
├── documents/
│   └── report.pdf
├── photos/
│   └── image.jpg
└── ...
```

This means: given *any single archive*, you can see every file ever tracked and know which volume holds each binary.

## Cache System

NGDAR uses the XDG Base Directory for an OS-level cache:

```
~/.cache/ngdar/<repository_id>/cache.tsv
```

Format: `[size] [mtime] [blake3_hash] [relative_path]`

If `size` and `mtime` match a cached entry, the BLAKE3 hash is reused without reading the file. If the OS purges the cache, NGDAR transparently recomputes hashes.

## Building

```bash
cargo build --release
cargo test
```

## License

MIT