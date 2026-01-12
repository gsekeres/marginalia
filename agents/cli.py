"""Command-line interface for Marginalia."""

import argparse
import asyncio
import sys
from pathlib import Path

from dotenv import load_dotenv
load_dotenv()  # Load .env file

from rich.console import Console
from rich.table import Table

from .models import PaperStatus
from .pdf_finder import PDFFinder, find_pdfs_batch
from .summarizer import Summarizer, summarize_batch
from .vault import VaultManager

console = Console()


def get_vault_path() -> Path:
    """Get the vault path from environment or default."""
    import os
    return Path(os.getenv("VAULT_PATH", "./vault"))


def cmd_status(args):
    """Show vault status."""
    vault = VaultManager(get_vault_path())
    vault.print_stats()


def cmd_import(args):
    """Import papers from BibTeX file."""
    vault = VaultManager(get_vault_path())
    bib_path = Path(args.file)

    if not bib_path.exists():
        console.print(f"[red]File not found: {bib_path}[/red]")
        return

    added = vault.import_bibtex(bib_path)
    console.print(f"[green]Imported {added} new papers[/green]")


def cmd_search(args):
    """Search for papers."""
    vault = VaultManager(get_vault_path())
    results = vault.search_papers(args.query)

    if not results:
        console.print(f"[yellow]No papers found matching '{args.query}'[/yellow]")
        return

    table = Table(title=f"Search Results: '{args.query}'")
    table.add_column("Citekey", style="cyan")
    table.add_column("Title")
    table.add_column("Year", justify="right")
    table.add_column("Status", style="green")

    for paper in results[:20]:
        table.add_row(
            paper.citekey,
            paper.title[:50] + "..." if len(paper.title) > 50 else paper.title,
            str(paper.year) if paper.year else "?",
            paper.status.value,
        )

    console.print(table)
    if len(results) > 20:
        console.print(f"[dim]... and {len(results) - 20} more[/dim]")


def cmd_want(args):
    """Mark papers as wanted for download."""
    vault = VaultManager(get_vault_path())

    if args.all:
        # Mark all discovered papers as wanted
        citekeys = [p.citekey for p in vault.index.get_papers_by_status(PaperStatus.DISCOVERED)]
    else:
        citekeys = args.citekeys

    count = vault.mark_wanted(citekeys)
    console.print(f"[green]Marked {count} papers as wanted[/green]")


def cmd_find(args):
    """Find and download PDFs for wanted papers."""
    vault = VaultManager(get_vault_path())
    wanted = vault.get_wanted_papers()

    if not wanted:
        console.print("[yellow]No papers marked as wanted[/yellow]")
        return

    console.print(f"[blue]Finding PDFs for {len(wanted)} papers...[/blue]")

    async def run():
        results = await find_pdfs_batch(wanted, vault.vault_path, limit=args.limit)

        # Update vault with results
        success = sum(1 for r in results if r.success)
        for result in results:
            paper = vault.get_paper(result.citekey)
            if paper:
                if result.success:
                    paper.status = PaperStatus.DOWNLOADED
                    paper.pdf_path = result.pdf_path
                else:
                    paper.manual_download_links = result.manual_links
                    paper.search_attempts += 1

        vault.save_index()
        console.print(f"\n[green]Downloaded {success}/{len(results)} papers[/green]")

        # Show papers needing manual download
        manual = vault.get_papers_needing_manual_download()
        if manual:
            console.print(f"[yellow]{len(manual)} papers need manual download[/yellow]")

    asyncio.run(run())


def cmd_summarize(args):
    """Summarize downloaded papers."""
    vault = VaultManager(get_vault_path())
    downloaded = vault.get_downloaded_papers()

    if not downloaded:
        console.print("[yellow]No downloaded papers to summarize[/yellow]")
        return

    console.print(f"[blue]Summarizing {len(downloaded)} papers...[/blue]")

    results = summarize_batch(downloaded, vault.vault_path, limit=args.limit)

    # Update vault
    success = sum(1 for r in results if r.success)
    for result in results:
        paper = vault.get_paper(result.citekey)
        if paper and result.success:
            paper.status = PaperStatus.SUMMARIZED
            paper.summary_path = result.summary_path

    vault.save_index()
    console.print(f"\n[green]Summarized {success}/{len(results)} papers[/green]")


def cmd_register(args):
    """Register a manually downloaded PDF."""
    vault = VaultManager(get_vault_path())
    pdf_path = Path(args.pdf)

    if not pdf_path.exists():
        console.print(f"[red]File not found: {pdf_path}[/red]")
        return

    if vault.register_pdf(args.citekey, pdf_path):
        console.print(f"[green]Registered PDF for {args.citekey}[/green]")
    else:
        console.print(f"[red]Paper not found: {args.citekey}[/red]")


def cmd_index(args):
    """Generate the Obsidian index page."""
    vault = VaultManager(get_vault_path())
    index_path = vault.generate_index_page()
    console.print(f"[green]Generated index at {index_path}[/green]")


def cmd_list(args):
    """List papers by status."""
    vault = VaultManager(get_vault_path())

    status = PaperStatus(args.status) if args.status else None
    papers = vault.index.get_papers_by_status(status) if status else list(vault.index.papers.values())

    table = Table(title=f"Papers ({args.status or 'all'})")
    table.add_column("Citekey", style="cyan")
    table.add_column("Title")
    table.add_column("Authors")
    table.add_column("Year", justify="right")
    table.add_column("Status", style="green")

    for paper in papers[:args.limit]:
        table.add_row(
            paper.citekey,
            paper.title[:40] + "..." if len(paper.title) > 40 else paper.title,
            paper.authors[0] if paper.authors else "?",
            str(paper.year) if paper.year else "?",
            paper.status.value,
        )

    console.print(table)
    if len(papers) > args.limit:
        console.print(f"[dim]Showing {args.limit} of {len(papers)} papers[/dim]")


def cmd_manual(args):
    """Show papers needing manual download."""
    vault = VaultManager(get_vault_path())
    manual = vault.get_papers_needing_manual_download()

    if not manual:
        console.print("[green]No papers need manual download![/green]")
        return

    console.print(f"[yellow]{len(manual)} papers need manual download:[/yellow]\n")

    for paper in manual:
        console.print(f"[bold cyan]{paper.citekey}[/bold cyan]")
        console.print(f"  {paper.title}")
        console.print(f"  {paper.authors_str} ({paper.year})")
        if paper.manual_download_links:
            console.print("  [blue]Search links:[/blue]")
            for link in paper.manual_download_links[:3]:
                console.print(f"    - {link}")
        console.print()


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="Marginalia - Agent-based academic literature management"
    )
    subparsers = parser.add_subparsers(dest="command", help="Available commands")

    # Status command
    subparsers.add_parser("status", help="Show vault status")

    # Import command
    import_parser = subparsers.add_parser("import", help="Import papers from BibTeX")
    import_parser.add_argument("file", help="Path to BibTeX file")

    # Search command
    search_parser = subparsers.add_parser("search", help="Search for papers")
    search_parser.add_argument("query", help="Search query")

    # Want command
    want_parser = subparsers.add_parser("want", help="Mark papers as wanted")
    want_parser.add_argument("citekeys", nargs="*", help="Citekeys to mark")
    want_parser.add_argument("--all", action="store_true", help="Mark all discovered papers")

    # Find command
    find_parser = subparsers.add_parser("find", help="Find and download PDFs")
    find_parser.add_argument("--limit", type=int, help="Maximum papers to process")

    # Summarize command
    summarize_parser = subparsers.add_parser("summarize", help="Summarize downloaded papers")
    summarize_parser.add_argument("--limit", type=int, help="Maximum papers to process")

    # Register command
    register_parser = subparsers.add_parser("register", help="Register a manually downloaded PDF")
    register_parser.add_argument("citekey", help="Paper citekey")
    register_parser.add_argument("pdf", help="Path to PDF file")

    # Index command
    subparsers.add_parser("index", help="Generate Obsidian index page")

    # List command
    list_parser = subparsers.add_parser("list", help="List papers")
    list_parser.add_argument("--status", choices=[s.value for s in PaperStatus], help="Filter by status")
    list_parser.add_argument("--limit", type=int, default=50, help="Maximum papers to show")

    # Manual command
    subparsers.add_parser("manual", help="Show papers needing manual download")

    args = parser.parse_args()

    if args.command is None:
        parser.print_help()
        return

    # Dispatch to command handler
    commands = {
        "status": cmd_status,
        "import": cmd_import,
        "search": cmd_search,
        "want": cmd_want,
        "find": cmd_find,
        "summarize": cmd_summarize,
        "register": cmd_register,
        "index": cmd_index,
        "list": cmd_list,
        "manual": cmd_manual,
    }

    if args.command in commands:
        commands[args.command](args)
    else:
        parser.print_help()


if __name__ == "__main__":
    main()
