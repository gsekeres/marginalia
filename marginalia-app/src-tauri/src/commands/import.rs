//! BibTeX import and export commands

use crate::models::{Paper, PaperStatus};
use crate::storage::PaperRepo;
use crate::AppState;
use std::fs;
use biblatex::{Bibliography, ChunksExt, PermissiveType};
use tauri::State;
use tracing::{info, warn};

#[derive(serde::Serialize)]
pub struct ImportResult {
    pub added: usize,
    pub updated: usize,
    pub source_path: String,
}

#[tauri::command]
pub async fn import_bibtex(
    _vault_path: String,
    bib_path: String,
    state: State<'_, AppState>,
) -> Result<ImportResult, String> {
    let bib_content = fs::read_to_string(&bib_path)
        .map_err(|e| format!("Failed to read BibTeX file: {}", e))?;

    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let paper_repo = PaperRepo::new(&db.conn);

    let mut added = 0;
    let mut updated = 0;

    // Parse BibTeX entries
    let entries = parse_bibtex(&bib_content)?;

    for entry in entries {
        if paper_repo.exists(&entry.citekey).unwrap_or(false) {
            paper_repo.update(&entry)
                .map_err(|e| format!("Failed to update paper: {}", e))?;
            updated += 1;
        } else {
            paper_repo.insert(&entry)
                .map_err(|e| format!("Failed to insert paper: {}", e))?;
            added += 1;
        }
    }

    info!("Imported {} new, {} updated papers from {}", added, updated, bib_path);

    Ok(ImportResult {
        added,
        updated,
        source_path: bib_path,
    })
}

fn parse_bibtex(content: &str) -> Result<Vec<Paper>, String> {
    let bibliography = Bibliography::parse(content)
        .map_err(|e| format!("Failed to parse BibTeX: {:?}", e))?;

    let mut papers = Vec::new();

    for entry in bibliography.iter() {
        let citekey = entry.key.to_string();

        // Get title - required field (use .ok() to convert Result to Option)
        let title = entry.title()
            .ok()
            .map(|chunks| format_chunks(chunks))
            .unwrap_or_else(|| {
                warn!("Entry '{}' has no title", citekey);
                String::new()
            });

        // Get authors
        let authors: Vec<String> = entry.author()
            .ok()
            .map(|persons| {
                persons.iter()
                    .map(|p| format_person(p))
                    .collect()
            })
            .unwrap_or_default();

        // Get year from date field
        let year = entry.date()
            .ok()
            .and_then(|date| extract_year(&date));

        // Get journal (also check booktitle for proceedings)
        let journal = entry.journal()
            .ok()
            .map(|chunks| format_chunks(chunks))
            .or_else(|| entry.book_title().ok().map(|chunks| format_chunks(chunks)));

        // Get DOI
        let doi = entry.doi().ok();

        // Get URL
        let url = entry.url().ok();

        // Get abstract
        let r#abstract = entry.abstract_()
            .ok()
            .map(|chunks| format_chunks(chunks));

        // Get volume
        let volume = entry.volume()
            .ok()
            .map(|v| format_permissive_i64(&v));

        // Get number/issue
        let number = entry.number()
            .ok()
            .map(|chunks| format_chunks(chunks));

        // Get pages
        let pages = entry.pages()
            .ok()
            .map(|p| format_pages(&p));

        let mut paper = Paper::new(citekey.clone(), title);
        paper.authors = authors;
        paper.year = year;
        paper.journal = journal;
        paper.doi = doi;
        paper.url = url;
        paper.r#abstract = r#abstract;
        paper.volume = volume;
        paper.number = number;
        paper.pages = pages;
        paper.status = PaperStatus::Discovered;

        papers.push(paper);
    }

    if papers.is_empty() {
        warn!("No valid entries found in BibTeX content");
    }

    Ok(papers)
}

/// Format biblatex Chunks into a clean string
fn format_chunks(chunks: &[biblatex::Spanned<biblatex::Chunk>]) -> String {
    chunks.format_verbatim()
        .replace("\n", " ")
        .trim()
        .to_string()
}

/// Extract year from a PermissiveType<Date>
fn extract_year(date: &PermissiveType<biblatex::Date>) -> Option<i32> {
    match date {
        PermissiveType::Typed(d) => {
            // Date has a value field which is a DateValue enum
            use biblatex::DateValue;
            match &d.value {
                DateValue::At(dt) => Some(dt.year),
                DateValue::After(dt) => Some(dt.year),
                DateValue::Before(dt) => Some(dt.year),
                DateValue::Between(start, _) => Some(start.year),
            }
        }
        PermissiveType::Chunks(_) => None,
    }
}

/// Format a PermissiveType<i64> to string
fn format_permissive_i64(v: &PermissiveType<i64>) -> String {
    match v {
        PermissiveType::Typed(n) => n.to_string(),
        PermissiveType::Chunks(c) => format_chunks(c),
    }
}

/// Format pages from PermissiveType<Vec<Range>>
fn format_pages(p: &PermissiveType<Vec<std::ops::Range<u32>>>) -> String {
    match p {
        PermissiveType::Typed(ranges) => {
            ranges.iter()
                .map(|r| {
                    if r.start == r.end || r.end == r.start + 1 {
                        r.start.to_string()
                    } else {
                        format!("{}-{}", r.start, r.end.saturating_sub(1))
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        }
        PermissiveType::Chunks(c) => format_chunks(c),
    }
}

/// Format a Person into "FirstName LastName" format
fn format_person(person: &biblatex::Person) -> String {
    let mut parts = Vec::new();

    if !person.given_name.is_empty() {
        parts.push(person.given_name.clone());
    }
    if !person.prefix.is_empty() {
        parts.push(person.prefix.clone());
    }
    if !person.name.is_empty() {
        parts.push(person.name.clone());
    }
    if !person.suffix.is_empty() {
        parts.push(person.suffix.clone());
    }

    parts.join(" ")
}

#[tauri::command]
pub async fn export_bibtex(
    _vault_path: String,
    output_path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let paper_repo = PaperRepo::new(&db.conn);
    let papers = paper_repo.get_all()
        .map_err(|e| format!("Failed to get papers: {}", e))?;

    let mut content = String::new();

    for paper in papers.values() {
        content.push_str(&format!("@article{{{},\n", paper.citekey));
        content.push_str(&format!("  title = {{{}}},\n", paper.title));

        if !paper.authors.is_empty() {
            content.push_str(&format!("  author = {{{}}},\n", paper.authors.join(" and ")));
        }
        if let Some(year) = paper.year {
            content.push_str(&format!("  year = {{{}}},\n", year));
        }
        if let Some(journal) = &paper.journal {
            content.push_str(&format!("  journal = {{{}}},\n", journal));
        }
        if let Some(doi) = &paper.doi {
            content.push_str(&format!("  doi = {{{}}},\n", doi));
        }
        if let Some(url) = &paper.url {
            content.push_str(&format!("  url = {{{}}},\n", url));
        }
        content.push_str("}\n\n");
    }

    fs::write(&output_path, content)
        .map_err(|e| format!("Failed to write BibTeX: {}", e))?;

    info!("Exported {} papers to {}", papers.len(), output_path);
    Ok(())
}
