use crate::models::{Paper, PaperStatus, VaultIndex};
use crate::utils::claude::is_claude_available;
use std::path::PathBuf;
use std::fs;
use reqwest::Client;
use chrono::Utc;

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
pub struct FindPdfResult {
    pub success: bool,
    pub pdf_path: Option<String>,
    pub source: Option<String>,
    pub manual_links: Vec<String>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn find_pdf(vault_path: String, citekey: String) -> Result<FindPdfResult, String> {
    let mut index = load_index(&vault_path)?;

    let paper = index.papers.get(&citekey)
        .ok_or_else(|| format!("Paper not found: {}", citekey))?
        .clone();

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Try different sources
    let mut pdf_url: Option<(String, String)> = None;

    // 1. Try Unpaywall (if DOI exists)
    if let Some(doi) = &paper.doi {
        if let Some(url) = try_unpaywall(&client, doi).await {
            pdf_url = Some((url, "unpaywall".to_string()));
        }
    }

    // 2. Try Semantic Scholar
    if pdf_url.is_none() {
        if let Some(doi) = &paper.doi {
            if let Some(url) = try_semantic_scholar_doi(&client, doi).await {
                pdf_url = Some((url, "semantic_scholar".to_string()));
            }
        }
    }

    if pdf_url.is_none() {
        if let Some(url) = try_semantic_scholar_title(&client, &paper.title).await {
            pdf_url = Some((url, "semantic_scholar".to_string()));
        }
    }

    // 3. Try Claude CLI (if available)
    if pdf_url.is_none() && is_claude_available() {
        if let Some(url) = try_claude_search(&paper).await {
            pdf_url = Some((url, "claude".to_string()));
        }
    }

    // If we found a URL, download the PDF
    if let Some((url, source)) = pdf_url {
        match download_pdf_from_url(&client, &vault_path, &citekey, &url).await {
            Ok(path) => {
                // Update paper status
                if let Some(p) = index.papers.get_mut(&citekey) {
                    p.status = PaperStatus::Downloaded;
                    p.pdf_path = Some(path.clone());
                    p.downloaded_at = Some(Utc::now());
                }
                save_index(&vault_path, &index)?;

                return Ok(FindPdfResult {
                    success: true,
                    pdf_path: Some(path),
                    source: Some(source),
                    manual_links: Vec::new(),
                    error: None,
                });
            }
            Err(e) => {
                // URL didn't work, continue to manual links
                eprintln!("Failed to download from {}: {}", url, e);
            }
        }
    }

    // Generate manual search links
    let manual_links = generate_search_links(&paper);

    // Update search attempts
    if let Some(p) = index.papers.get_mut(&citekey) {
        p.search_attempts += 1;
        p.manual_download_links = manual_links.clone();
    }
    save_index(&vault_path, &index)?;

    Ok(FindPdfResult {
        success: false,
        pdf_path: None,
        source: None,
        manual_links,
        error: Some("No open access PDF found".to_string()),
    })
}

async fn try_unpaywall(client: &Client, doi: &str) -> Option<String> {
    let email = std::env::var("UNPAYWALL_EMAIL").unwrap_or_else(|_| "marginalia@example.com".to_string());
    let url = format!("https://api.unpaywall.org/v2/{}?email={}", doi, email);

    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let data: serde_json::Value = resp.json().await.ok()?;

    // Try best_oa_location first
    if let Some(best_loc) = data.get("best_oa_location") {
        if let Some(pdf_url) = best_loc.get("url_for_pdf").and_then(|v| v.as_str()) {
            if !pdf_url.is_empty() {
                return Some(pdf_url.to_string());
            }
        }
    }

    // Try oa_locations array
    if let Some(locations) = data.get("oa_locations").and_then(|v| v.as_array()) {
        for loc in locations {
            if let Some(pdf_url) = loc.get("url_for_pdf").and_then(|v| v.as_str()) {
                if !pdf_url.is_empty() {
                    return Some(pdf_url.to_string());
                }
            }
        }
    }

    None
}

async fn try_semantic_scholar_doi(client: &Client, doi: &str) -> Option<String> {
    let url = format!(
        "https://api.semanticscholar.org/graph/v1/paper/DOI:{}?fields=openAccessPdf",
        doi
    );

    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let data: serde_json::Value = resp.json().await.ok()?;

    data.get("openAccessPdf")
        .and_then(|v| v.get("url"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

async fn try_semantic_scholar_title(client: &Client, title: &str) -> Option<String> {
    let encoded_title = urlencoding::encode(title);
    let url = format!(
        "https://api.semanticscholar.org/graph/v1/paper/search?query={}&fields=openAccessPdf&limit=1",
        encoded_title
    );

    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let data: serde_json::Value = resp.json().await.ok()?;

    data.get("data")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|paper| paper.get("openAccessPdf"))
        .and_then(|v| v.get("url"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

async fn try_claude_search(paper: &Paper) -> Option<String> {
    use std::process::Command;

    let prompt = format!(
        "Find a direct download URL for the open-access PDF of this academic paper. \
         Only respond with a URL that ends in .pdf, nothing else. If you cannot find one, respond with exactly 'NONE'.\n\n\
         Title: {}\nAuthors: {}\nYear: {:?}\nDOI: {:?}",
        paper.title,
        paper.authors.join(", "),
        paper.year,
        paper.doi
    );

    let output = Command::new("claude")
        .args(["--print", "-p", &prompt])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let response = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if response.starts_with("http") && response.contains(".pdf") {
        Some(response)
    } else {
        None
    }
}

async fn download_pdf_from_url(
    client: &Client,
    vault_path: &str,
    citekey: &str,
    url: &str,
) -> Result<String, String> {
    let resp = client.get(url)
        .header("User-Agent", "Marginalia/1.0")
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP status: {}", resp.status()));
    }

    let content_type = resp.headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !content_type.contains("pdf") && !url.ends_with(".pdf") {
        return Err("Response is not a PDF".to_string());
    }

    let bytes = resp.bytes().await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    // Create paper directory
    let paper_dir = PathBuf::from(vault_path).join("papers").join(citekey);
    fs::create_dir_all(&paper_dir)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    let pdf_path = paper_dir.join("paper.pdf");
    fs::write(&pdf_path, &bytes)
        .map_err(|e| format!("Failed to write PDF: {}", e))?;

    Ok(format!("papers/{}/paper.pdf", citekey))
}

fn generate_search_links(paper: &Paper) -> Vec<String> {
    let mut links = Vec::new();

    let encoded_title = urlencoding::encode(&paper.title);
    let first_author = paper.authors.first().map(|s| s.as_str()).unwrap_or("");
    let encoded_author = urlencoding::encode(first_author);

    // Google Scholar
    links.push(format!(
        "https://scholar.google.com/scholar?q={}",
        encoded_title
    ));

    // Semantic Scholar
    links.push(format!(
        "https://www.semanticscholar.org/search?q={}",
        encoded_title
    ));

    // DOI link
    if let Some(doi) = &paper.doi {
        links.push(format!("https://doi.org/{}", doi));
    }

    // SSRN search
    links.push(format!(
        "https://papers.ssrn.com/sol3/results.cfm?txtKey_Words={}",
        encoded_title
    ));

    // Author search
    if !first_author.is_empty() {
        links.push(format!(
            "https://scholar.google.com/scholar?q=author:{}+{}",
            encoded_author, encoded_title
        ));
    }

    links
}

#[tauri::command]
pub async fn download_pdf(
    vault_path: String,
    citekey: String,
    url: String,
) -> Result<String, String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let path = download_pdf_from_url(&client, &vault_path, &citekey, &url).await?;

    // Update index
    let mut index = load_index(&vault_path)?;
    if let Some(paper) = index.papers.get_mut(&citekey) {
        paper.status = PaperStatus::Downloaded;
        paper.pdf_path = Some(path.clone());
        paper.downloaded_at = Some(Utc::now());
    }
    save_index(&vault_path, &index)?;

    Ok(path)
}
