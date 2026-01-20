/**
 * TypeScript types for Marginalia
 * These mirror the Rust models in src-tauri/src/models/
 */

// Paper status enum
export type PaperStatus =
  | 'discovered'
  | 'wanted'
  | 'queued'
  | 'downloaded'
  | 'summarized'
  | 'failed';

// Citation reference
export interface Citation {
  citekey: string;
  title: string | null;
  authors: string | null;
  year: number | null;
  doi: string | null;
  status: string;
}

// Related paper from summarization
export interface RelatedPaper {
  title: string;
  authors: string[];
  year: number | null;
  why_related: string;
  vault_citekey: string | null;
}

// Main paper interface
export interface Paper {
  citekey: string;
  title: string;
  authors: string[];
  year: number | null;
  journal: string | null;
  volume: string | null;
  number: string | null;
  pages: string | null;
  doi: string | null;
  url: string | null;
  abstract: string | null;

  status: PaperStatus;

  pdf_path: string | null;
  summary_path: string | null;
  notes_path: string | null;

  added_at: string;
  downloaded_at: string | null;
  summarized_at: string | null;

  citations: Citation[];
  cited_by: string[];
  related_papers: RelatedPaper[];

  search_attempts: number;
  last_search_error: string | null;
  manual_download_links: string[];
}

// Vault statistics
export interface VaultStats {
  total: number;
  discovered: number;
  wanted: number;
  queued: number;
  downloaded: number;
  summarized: number;
  failed: number;
}

// Job types
export type JobType =
  | 'import_bib'
  | 'find_pdf'
  | 'download_pdf'
  | 'extract_text'
  | 'summarize'
  | 'build_graph';

// Job status
export type JobStatus =
  | 'pending'
  | 'running'
  | 'completed'
  | 'failed'
  | 'cancelled';

// Job record
export interface Job {
  id: string;
  job_type: JobType;
  citekey: string | null;
  status: JobStatus;
  progress: number;
  error: string | null;
  log_path: string | null;
  started_at: string | null;
  finished_at: string | null;
  created_at: string;
}

// Job update event payload
export interface JobUpdate {
  id: string;
  status: string;
  progress: number;
  error: string | null;
}

// Claude CLI status
export interface ClaudeStatus {
  available: boolean;
  version: string | null;
  logged_in: boolean;
}

// Find PDF result
export interface FindPdfResult {
  success: boolean;
  pdf_path: string | null;
  source: string | null;
  manual_links: string[];
  error: string | null;
}

// PDF search progress event
export interface PdfSearchProgress {
  citekey: string;
  progress: number;
  current_source: string | null;
  message: string;
}

// Summary result
export interface SummaryResult {
  success: boolean;
  summary_path: string | null;
  raw_response_path: string | null;
  error: string | null;
}

// Paper notes
export interface Highlight {
  id: string;
  page: number;
  text: string;
  color: string;
  rect: [number, number, number, number];
  created_at: string;
}

export interface PaperNotes {
  citekey: string;
  content: string;
  highlights: Highlight[];
  updated_at: string;
}

// Graph data for vis.js
export interface GraphNode {
  id: string;
  label: string;
  title?: string;
  color?: {
    background: string;
    border: string;
  };
  font?: {
    color: string;
  };
}

export interface GraphEdge {
  id: string;
  from: string;
  to: string;
  label?: string;
  arrows?: string;
  dashes?: boolean;
  color?: {
    color: string;
  };
}

export interface GraphData {
  nodes: GraphNode[];
  edges: GraphEdge[];
}

// Query options for paper listing
export interface PaperQuery {
  status?: PaperStatus;
  search?: string;
  limit?: number;
  offset?: number;
}

// Settings
export interface Settings {
  claude_model?: string;
  pdf_sources?: string[];
  auto_find_pdf?: boolean;
  auto_summarize?: boolean;
}

// Project for organizing papers
export interface Project {
  id: string;
  name: string;
  color: string;
  description: string | null;
  created_at: string;
  updated_at: string;
  paper_count: number;
}

// Request types for project commands
export interface CreateProjectRequest {
  name: string;
  color?: string;
  description?: string;
}

export interface UpdateProjectRequest {
  id: string;
  name: string;
  color: string;
  description?: string;
}
