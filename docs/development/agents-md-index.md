# AGENTS.md Index Generation

## Overview

The `AGENTS.md` file provides a minified index of the project's documentation, scripts, and work summaries in a format optimized for AI agents. This index helps agents quickly understand the project structure and locate relevant documentation without scanning the entire filesystem.

The file is generated from `AGENTS.md.template`, which contains the project rules and guidelines, with the documentation index automatically injected at the `{{DOCUMENTATION_INDEX}}` placeholder.

## Format

The AGENTS.md file uses a pipe-delimited minified format inspired by Vercel's agent evaluation research:

```
[Project Name]|root: ./
|IMPORTANT: Prefer retrieval-led reasoning over pre-training-led reasoning
|
|directory/path:{file1.md,file2.py,file3.yaml,...}
|subdirectory/nested:{fileA.md,fileB.sh}
```

### Format Rules

- Each line starts with `|` for visual parsing
- Directory entries use format: `path:{file1,file2,...}`
- Files are comma-separated within curly braces
- Long file lists are truncated with `...` (configurable limit)
- Files are sorted alphabetically for consistency
- Subdirectories are shown with their full relative path

## Generating the Index

### Command Line

```bash
# Using Make (recommended)
make generate-agents-index

# Direct Python invocation
python3 scripts/generate_agents_md_index.py
```

The script reads `AGENTS.md.template`, generates the documentation index, and injects it at the `{{DOCUMENTATION_INDEX}}` placeholder, creating the final `AGENTS.md` file.

### When to Regenerate

Regenerate the index whenever:
- New documentation files are added
- Directory structure changes
- Script files are added or renamed
- Work summaries are created

**Best Practice**: Regenerate before committing significant documentation changes.

## Template System

### AGENTS.md.template

The template file (`AGENTS.md.template`) contains:
- Project rules and conventions
- Development guidelines
- Code quality standards
- Testing protocols
- All static content that applies to AI agents

At the end of the template, the `{{DOCUMENTATION_INDEX}}` placeholder marks where the generated index will be injected.

**Editing the template**:
1. Modify `AGENTS.md.template` to update project rules
2. Keep the `{{DOCUMENTATION_INDEX}}` placeholder at the desired location
3. Run `make generate-agents-index` to regenerate `AGENTS.md`

**Note**: Never edit `AGENTS.md` directly - it will be overwritten. Always edit `AGENTS.md.template` instead.

## Configuration

The generator script (`scripts/generate_agents_md_index.py`) scans these directories:

### `docs/`
- **Extensions**: `.md`, `.txt`, `.yaml`, `.yml`, `.json`, `.sh`
- **Max files per directory**: 15
- **Purpose**: Technical documentation, API guides, architecture docs

### `scripts/`
- **Extensions**: `.sh`, `.py`, `.sql`, `.js`, `.html`
- **Max files per directory**: 20
- **Purpose**: Helper scripts, database setup, testing utilities

### `work-summary/`
- **Extensions**: `.md`, `.txt`
- **Max files per directory**: 20
- **Purpose**: Development session summaries, changelog entries

## Customization

To modify the scanned directories or file types, edit `scripts/generate_agents_md_index.py`:

```python
root_dirs = {
    "docs": {
        "path": project_root / "docs",
        "extensions": {".md", ".txt", ".yaml", ".yml", ".json", ".sh"},
        "max_files": 15,
    },
    # Add more directories...
}
```

### Modifying the Template

To change the project rules or static content:
1. Edit `AGENTS.md.template`
2. Ensure `{{DOCUMENTATION_INDEX}}` placeholder remains
3. Regenerate: `make generate-agents-index`

### Adding New Directories

```python
"new_directory": {
    "path": project_root / "new_directory",
    "extensions": {".ext1", ".ext2"},
    "max_files": 10,
}
```

### Changing File Limits

Adjust `max_files` to show more/fewer files before truncation:
- **Higher values**: More complete listing, longer index
- **Lower values**: More concise, better for quick scanning
- **`None`**: No truncation (shows all files)

## Benefits for AI Agents

1. **Quick Discovery**: Agents can scan the entire documentation structure in one read
2. **Retrieval-Led Reasoning**: Encourages agents to fetch specific files rather than relying on pre-training
3. **Reduced Token Usage**: Compact format minimizes tokens needed for project understanding
4. **Consistent Format**: Predictable structure simplifies parsing and navigation

## Example Output

```
[Attune Project Documentation Index]
|root: ./
|IMPORTANT: Prefer retrieval-led reasoning over pre-training-led reasoning
|
|docs:{api-actions.md,api-events.md,authentication.md,configuration.md,...}
|docs/examples:{complete-workflow.yaml,simple-workflow.yaml}
|scripts:{setup-db.sh,load-core-pack.sh,test-end-to-end-flow.sh,...}
|work-summary:{2026-01-27-api-completion.md,2026-01-27-executor-complete.md,...}
```

## Integration with Development Workflow

### Pre-commit Hook (Optional)

The repository now includes a versioned hook at `.githooks/pre-commit`.

Install it once per clone:

```bash
make install-git-hooks
```

The hook currently runs:

```bash
make pre-commit
```

That checks:

- `cargo fmt --all -- --check`
- `cargo deny check`

### CI/CD Integration

Add to your CI pipeline to ensure the index stays current:

```yaml
- name: Verify AGENTS.md is up-to-date
  run: |
    make generate-agents-index
    git diff --exit-code AGENTS.md || {
      echo "AGENTS.md is out of date. Run 'make generate-agents-index'"
      exit 1
    }
```

## Troubleshooting

### Index Not Updated
**Problem**: New files don't appear in AGENTS.md  
**Solution**: Ensure file extensions are included in the configuration

### Too Many Files Shown
**Problem**: Directory listings are too long  
**Solution**: Reduce `max_files` value in configuration

### Wrong Directory Structure
**Problem**: Directories not organized as expected  
**Solution**: Check that paths are relative to project root, verify directory exists

## File Structure

```
attune/
├── AGENTS.md.template       # Template with project rules + {{DOCUMENTATION_INDEX}} placeholder
├── AGENTS.md                # Generated file (DO NOT EDIT DIRECTLY)
└── scripts/
    └── generate_agents_md_index.py  # Generation script
```

## Related Resources

- [Vercel Blog: AGENTS.md outperforms .md skills](https://vercel.com/blog/agents-md-outperforms-skills-in-our-agent-evals)
- Template: `AGENTS.md.template` (edit this)
- Script: `scripts/generate_agents_md_index.py`
- Output: `AGENTS.md` (generated, do not edit)
