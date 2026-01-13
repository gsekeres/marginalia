use crate::models::{Paper, PaperStatus, VaultIndex};
use std::path::PathBuf;
use std::fs;
use regex::Regex;

const INDEX_FILENAME: &str = ".marginalia_index.json";

fn load_index(vault_path: &str) -> Result<VaultIndex, String> {
    let path = PathBuf::from(vault_path).join(INDEX_FILENAME);
    if !path.exists() {
        return Ok(VaultIndex::new());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read index: {}", e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse index: {}", e))
}

fn save_index(vault_path: &str, index: &VaultIndex) -> Result<(), String> {
    let path = PathBuf::from(vault_path).join(INDEX_FILENAME);
    let content = serde_json::to_string_pretty(index)
        .map_err(|e| format!("Failed to serialize index: {}", e))?;
    fs::write(&path, content)
        .map_err(|e| format!("Failed to write index: {}", e))
}

#[derive(serde::Serialize)]
pub struct ImportResult {
    pub added: usize,
    pub updated: usize,
    pub source_path: String,
}

#[tauri::command]
pub async fn import_bibtex(vault_path: String, bib_path: String) -> Result<ImportResult, String> {
    let bib_content = fs::read_to_string(&bib_path)
        .map_err(|e| format!("Failed to read BibTeX file: {}", e))?;

    let mut index = load_index(&vault_path)?;
    let mut added = 0;
    let mut updated = 0;

    // Parse BibTeX entries
    let entries = parse_bibtex(&bib_content)?;

    for entry in entries {
        if index.papers.contains_key(&entry.citekey) {
            updated += 1;
        } else {
            added += 1;
        }
        index.papers.insert(entry.citekey.clone(), entry);
    }

    // Store source path
    index.source_bib_path = Some(bib_path.clone());

    save_index(&vault_path, &index)?;

    Ok(ImportResult {
        added,
        updated,
        source_path: bib_path,
    })
}

fn parse_bibtex(content: &str) -> Result<Vec<Paper>, String> {
    let mut papers = Vec::new();

    // Regex to match BibTeX entries
    let entry_re = Regex::new(r"@(\w+)\s*\{\s*([^,]+)\s*,([^@]*)\}").unwrap();

    for cap in entry_re.captures_iter(content) {
        let _entry_type = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let citekey = cap.get(2).map(|m| m.as_str().trim()).unwrap_or("").to_string();
        let fields_str = cap.get(3).map(|m| m.as_str()).unwrap_or("");

        if citekey.is_empty() {
            continue;
        }

        let fields = parse_bibtex_fields(fields_str);

        let title = fields.get("title")
            .map(|s| clean_bibtex_string(s))
            .unwrap_or_default();

        let authors = fields.get("author")
            .map(|s| parse_authors(s))
            .unwrap_or_default();

        let year = fields.get("year")
            .and_then(|s| s.trim().parse::<i32>().ok());

        let journal = fields.get("journal")
            .map(|s| clean_bibtex_string(s));

        let doi = fields.get("doi")
            .map(|s| clean_bibtex_string(s));

        let url = fields.get("url")
            .map(|s| clean_bibtex_string(s));

        let r#abstract = fields.get("abstract")
            .map(|s| clean_bibtex_string(s));

        let volume = fields.get("volume")
            .map(|s| clean_bibtex_string(s));

        let number = fields.get("number")
            .map(|s| clean_bibtex_string(s));

        let pages = fields.get("pages")
            .map(|s| clean_bibtex_string(s));

        let mut paper = Paper::new(citekey, title);
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

    Ok(papers)
}

fn parse_bibtex_fields(content: &str) -> std::collections::HashMap<String, String> {
    let mut fields = std::collections::HashMap::new();

    // Simple field parser
    let field_re = Regex::new(r"(\w+)\s*=\s*\{([^}]*)\}").unwrap();

    for cap in field_re.captures_iter(content) {
        let key = cap.get(1).map(|m| m.as_str().to_lowercase()).unwrap_or_default();
        let value = cap.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
        fields.insert(key, value);
    }

    fields
}

fn clean_bibtex_string(s: &str) -> String {
    s.replace("{", "")
     .replace("}", "")
     .replace("\\", "")
     .replace("\n", " ")
     .trim()
     .to_string()
}

fn parse_authors(s: &str) -> Vec<String> {
    s.split(" and ")
     .map(|a| clean_bibtex_string(a.trim()))
     .filter(|a| !a.is_empty())
     .collect()
}

#[tauri::command]
pub async fn export_bibtex(vault_path: String, output_path: String) -> Result<(), String> {
    let index = load_index(&vault_path)?;

    let mut content = String::new();

    for paper in index.papers.values() {
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

    Ok(())
}
