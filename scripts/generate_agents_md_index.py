#!/usr/bin/env python3
"""
Generate AGENTS.md index file in minified format.

This script scans the docs, scripts, and work-summary directories
and generates a minified index file that helps AI agents quickly
understand the project structure and available documentation.

The script uses AGENTS.md.template as a base and injects the generated
index at the {{DOCUMENTATION_INDEX}} placeholder.

Usage:
    python scripts/generate_agents_md_index.py
"""

import os
from collections import defaultdict
from pathlib import Path
from typing import Dict, List, Set


def get_project_root() -> Path:
    """Get the project root directory (parent of scripts/)."""
    script_dir = Path(__file__).parent
    return script_dir.parent


def scan_directory(
    base_path: Path, extensions: Set[str] = None
) -> Dict[str, List[str]]:
    """
    Scan a directory and organize files by subdirectory.

    Args:
        base_path: Directory to scan
        extensions: Set of file extensions to include (e.g., {'.md', '.py'}). None means all files.

    Returns:
        Dictionary mapping relative directory paths to lists of filenames
    """
    if not base_path.exists():
        return {}

    structure = defaultdict(list)

    for item in sorted(base_path.rglob("*")):
        if item.is_file():
            # Filter by extension if specified
            if extensions and item.suffix not in extensions:
                continue

            # Get relative path from base_path
            rel_path = item.relative_to(base_path)
            parent_dir = str(rel_path.parent) if rel_path.parent != Path(".") else ""

            structure[parent_dir].append(item.name)

    return structure


def format_directory_entry(
    dir_path: str, files: List[str], max_files: int = None
) -> str:
    """
    Format a directory entry in minified format.

    Args:
        dir_path: Directory path (empty string for root)
        files: List of filenames in the directory
        max_files: Maximum number of files to list before truncating

    Returns:
        Formatted string like "path:{file1,file2,...}"
    """
    if not files:
        return ""

    # Sort files for consistency
    sorted_files = sorted(files)

    # Truncate if needed
    if max_files and len(sorted_files) > max_files:
        file_list = sorted_files[:max_files] + ["..."]
    else:
        file_list = sorted_files

    files_str = ",".join(file_list)

    if dir_path:
        return f"{dir_path}:{{{files_str}}}"
    else:
        return f"root:{{{files_str}}}"


def generate_index_content(root_dirs: Dict[str, Dict[str, any]]) -> str:
    """
    Generate the documentation index content.

    Args:
        root_dirs: Dictionary mapping directory names to their scan configs

    Returns:
        Formatted index content as a string
    """
    lines = []

    lines.append("[Attune Project Documentation Index]")
    lines.append("|root: ./")
    lines.append(
        "|IMPORTANT: Prefer retrieval-led reasoning over pre-training-led reasoning"
    )
    lines.append(
        "|IMPORTANT: This index provides a quick overview - use grep/read_file for details"
    )
    lines.append("|")
    lines.append("| Format: path/to/dir:{file1,file2,...}")
    lines.append(
        "| '...' indicates truncated file list - use grep/list_directory for full contents"
    )
    lines.append("|")
    lines.append("| To regenerate this index: make generate-agents-index")
    lines.append("|")

    # Process each root directory
    for dir_name, config in root_dirs.items():
        base_path = config["path"]
        extensions = config.get("extensions")
        max_files = config.get("max_files", 10)

        structure = scan_directory(base_path, extensions)

        if not structure:
            lines.append(f"|{dir_name}: (empty)")
            continue

        # Sort directories for consistent output
        sorted_dirs = sorted(structure.keys())

        for dir_path in sorted_dirs:
            files = structure[dir_path]

            # Build the full path relative to project root
            if dir_path:
                full_path = f"{dir_name}/{dir_path}"
            else:
                full_path = dir_name

            entry = format_directory_entry(full_path, files, max_files)
            if entry:
                lines.append(f"|{entry}")

    return "\n".join(lines)


def generate_agents_md(
    template_path: Path, output_path: Path, root_dirs: Dict[str, Dict[str, any]]
) -> None:
    """
    Generate the AGENTS.md file using template.

    Args:
        template_path: Path to AGENTS.md.template file
        output_path: Path where AGENTS.md should be written
        root_dirs: Dictionary mapping directory names to their scan configs
    """
    # Generate the index content
    index_content = generate_index_content(root_dirs)

    # Read the template
    if not template_path.exists():
        print(f"⚠️  Template not found at {template_path}")
        print(f"   Creating AGENTS.md without template...")
        content = index_content + "\n"
    else:
        template = template_path.read_text()
        # Inject the index into the template
        content = template.replace("{{DOCUMENTATION_INDEX}}", index_content)

    # Write to file
    output_path.write_text(content)
    print(f"✓ Generated {output_path}")
    index_lines = index_content.count("\n") + 1
    total_lines = content.count("\n") + 1
    print(f"  Index lines: {index_lines}")
    print(f"  Total lines: {total_lines}")


def main():
    """Main entry point."""
    project_root = get_project_root()

    # Configuration for directories to scan
    root_dirs = {
        "docs": {
            "path": project_root / "docs",
            "extensions": {".md", ".txt", ".yaml", ".yml", ".json", ".sh"},
            "max_files": 15,
        },
        "scripts": {
            "path": project_root / "scripts",
            "extensions": {".sh", ".py", ".sql", ".js", ".html"},
            "max_files": 20,
        },
        "work-summary": {
            "path": project_root / "work-summary",
            "extensions": {".md", ".txt"},
            "max_files": 20,
        },
    }

    template_path = project_root / "AGENTS.md.template"
    output_path = project_root / "AGENTS.md"

    print("Generating AGENTS.md index...")
    print(f"Project root: {project_root}")
    print(f"Template: {template_path}")
    print()

    # Generate the index
    generate_agents_md(template_path, output_path, root_dirs)

    print()
    print("Index generation complete!")
    print(f"Review the generated file at: {output_path}")


if __name__ == "__main__":
    main()
