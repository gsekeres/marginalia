#!/usr/bin/env python3
"""Quick import script - imports the references.bib and shows status."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))

from agents.vault import VaultManager

def main():
    vault_path = Path(__file__).parent.parent / "vault"
    bib_path = Path(__file__).parent.parent / "references.bib"

    print(f"Vault path: {vault_path}")
    print(f"BibTeX path: {bib_path}")

    if not bib_path.exists():
        print(f"Error: {bib_path} not found")
        return

    manager = VaultManager(vault_path)

    # Import
    added = manager.import_bibtex(bib_path)
    print(f"\nImported {added} papers")

    # Show stats
    print("\nVault Statistics:")
    manager.print_stats()

    # Generate index
    index_path = manager.generate_index_page()
    print(f"\nGenerated index at: {index_path}")

if __name__ == "__main__":
    main()
