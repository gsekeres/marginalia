/**
 * Centralized API client for Tauri commands
 *
 * This module wraps all Tauri invoke calls with proper typing
 * and error handling.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  Paper,
  PaperQuery,
  VaultStats,
  Job,
  JobUpdate,
  ClaudeStatus,
  FindPdfResult,
  PdfSearchProgress,
  SummaryResult,
  PaperNotes,
  GraphData,
  Settings,
  Project,
  CreateProjectRequest,
  UpdateProjectRequest,
} from '../types';

/**
 * Vault API
 */
export const vault = {
  /**
   * Open a vault at the given path
   */
  open: (path: string) =>
    invoke<VaultStats>('open_vault', { path }),

  /**
   * Create a new vault at the given path
   */
  create: (path: string) =>
    invoke<void>('create_vault', { path }),

  /**
   * Get recently opened vaults
   */
  getRecent: () =>
    invoke<string[]>('get_recent_vaults'),

  /**
   * Add a vault to recent list
   */
  addRecent: (path: string) =>
    invoke<void>('add_recent_vault', { path }),

  /**
   * Get vault statistics
   */
  getStats: (vaultPath: string) =>
    invoke<VaultStats>('get_vault_stats', { vaultPath }),

  /**
   * Find .bib files in a directory
   */
  findBibFiles: (path: string) =>
    invoke<string[]>('find_bib_files', { path }),
};

/**
 * Papers API
 */
export const papers = {
  /**
   * Get papers with optional filtering
   */
  list: (vaultPath: string, options?: PaperQuery) =>
    invoke<Paper[]>('get_papers', { vaultPath, ...options }),

  /**
   * Get a single paper by citekey
   */
  get: (vaultPath: string, citekey: string) =>
    invoke<Paper | null>('get_paper', { vaultPath, citekey }),

  /**
   * Update paper status
   */
  updateStatus: (vaultPath: string, citekey: string, status: string) =>
    invoke<void>('update_paper_status', { vaultPath, citekey, status }),

  /**
   * Search papers by title/author
   */
  search: (vaultPath: string, query: string) =>
    invoke<Paper[]>('search_papers', { vaultPath, query }),

  /**
   * Add a related paper reference
   */
  addRelated: (vaultPath: string, citekey: string, related: {
    title: string;
    authors: string[];
    year?: number;
    why_related: string;
  }) =>
    invoke<void>('add_related_paper', { vaultPath, citekey, ...related }),
};

/**
 * Import/Export API
 */
export const importExport = {
  /**
   * Import papers from a BibTeX file
   */
  importBibtex: (vaultPath: string, bibPath: string) =>
    invoke<number>('import_bibtex', { vaultPath, bibPath }),

  /**
   * Export papers to BibTeX
   */
  exportBibtex: (vaultPath: string, outputPath: string) =>
    invoke<void>('export_bibtex', { vaultPath, outputPath }),
};

/**
 * PDF API
 */
export const pdf = {
  /**
   * Find and download PDF for a paper
   */
  find: (vaultPath: string, citekey: string) =>
    invoke<FindPdfResult>('find_pdf', { vaultPath, citekey }),

  /**
   * Download PDF from a specific URL
   */
  download: (vaultPath: string, citekey: string, url: string) =>
    invoke<string>('download_pdf', { vaultPath, citekey, url }),

  /**
   * Subscribe to PDF search progress events
   */
  onSearchProgress: (callback: (progress: PdfSearchProgress) => void): Promise<UnlistenFn> =>
    listen<PdfSearchProgress>('pdf:search:progress', (event) => callback(event.payload)),
};

/**
 * Claude/Summarization API
 */
export const claude = {
  /**
   * Check Claude CLI status
   */
  checkStatus: () =>
    invoke<ClaudeStatus>('check_claude_cli'),

  /**
   * Summarize a paper
   */
  summarize: (vaultPath: string, citekey: string) =>
    invoke<SummaryResult>('summarize_paper', { vaultPath, citekey }),

  /**
   * Read raw response file (saved when summarization parse fails)
   */
  readRawResponse: (vaultPath: string, citekey: string) =>
    invoke<string>('read_raw_response', { vaultPath, citekey }),
};

/**
 * Notes API
 */
export const notes = {
  /**
   * Get notes for a paper
   */
  get: (vaultPath: string, citekey: string) =>
    invoke<PaperNotes | null>('get_notes', { vaultPath, citekey }),

  /**
   * Save notes for a paper
   */
  save: (vaultPath: string, citekey: string, content: string) =>
    invoke<void>('save_notes', { vaultPath, citekey, content }),

  /**
   * Add a highlight
   */
  addHighlight: (vaultPath: string, citekey: string, highlight: {
    page: number;
    text: string;
    color: string;
    rect: [number, number, number, number];
  }) =>
    invoke<string>('add_highlight', { vaultPath, citekey, highlight }),

  /**
   * Delete a highlight
   */
  deleteHighlight: (vaultPath: string, citekey: string, highlightId: string) =>
    invoke<void>('delete_highlight', { vaultPath, citekey, highlightId }),
};

/**
 * Graph API
 */
export const graph = {
  /**
   * Get graph data for visualization
   */
  get: (vaultPath: string) =>
    invoke<GraphData>('get_graph', { vaultPath }),

  /**
   * Connect two papers
   */
  connect: (vaultPath: string, fromCitekey: string, toCitekey: string, label?: string) =>
    invoke<void>('connect_papers', { vaultPath, fromCitekey, toCitekey, label }),

  /**
   * Disconnect two papers
   */
  disconnect: (vaultPath: string, fromCitekey: string, toCitekey: string) =>
    invoke<void>('disconnect_papers', { vaultPath, fromCitekey, toCitekey }),
};

/**
 * Jobs API
 */
export const jobs = {
  /**
   * Start a new job
   */
  start: (jobType: string, citekey?: string) =>
    invoke<string>('start_job', { jobType, citekey }),

  /**
   * Get job by ID
   */
  get: (jobId: string) =>
    invoke<Job | null>('get_job', { jobId }),

  /**
   * List jobs
   */
  list: (status?: string, limit?: number) =>
    invoke<Job[]>('list_jobs', { status, limit }),

  /**
   * List active (pending/running) jobs
   */
  listActive: () =>
    invoke<Job[]>('list_active_jobs'),

  /**
   * Cancel a job
   */
  cancel: (jobId: string) =>
    invoke<boolean>('cancel_job', { jobId }),

  /**
   * Subscribe to job updates
   */
  onUpdate: (callback: (update: JobUpdate) => void): Promise<UnlistenFn> =>
    listen<JobUpdate>('job:updated', (event) => callback(event.payload)),
};

/**
 * Settings API
 */
export const settings = {
  /**
   * Get current settings
   */
  get: () =>
    invoke<Settings>('get_settings'),

  /**
   * Save settings
   */
  save: (settings: Settings) =>
    invoke<void>('save_settings', { settings }),
};

/**
 * Projects API
 */
export const projects = {
  /**
   * List all projects
   */
  list: () =>
    invoke<Project[]>('list_projects'),

  /**
   * Get a single project by ID
   */
  get: (id: string) =>
    invoke<Project | null>('get_project', { id }),

  /**
   * Create a new project
   */
  create: (request: CreateProjectRequest) =>
    invoke<Project>('create_project', { request }),

  /**
   * Update a project
   */
  update: (request: UpdateProjectRequest) =>
    invoke<void>('update_project', { request }),

  /**
   * Delete a project
   */
  delete: (id: string) =>
    invoke<void>('delete_project', { id }),

  /**
   * Add a paper to a project
   */
  addPaper: (projectId: string, citekey: string) =>
    invoke<void>('add_paper_to_project', { projectId, citekey }),

  /**
   * Remove a paper from a project
   */
  removePaper: (projectId: string, citekey: string) =>
    invoke<void>('remove_paper_from_project', { projectId, citekey }),

  /**
   * Get all paper citekeys in a project
   */
  getPapers: (projectId: string) =>
    invoke<string[]>('get_project_papers', { projectId }),

  /**
   * Get all projects a paper belongs to
   */
  getPaperProjects: (citekey: string) =>
    invoke<Project[]>('get_paper_projects', { citekey }),

  /**
   * Set the projects for a paper (replaces existing)
   */
  setPaperProjects: (citekey: string, projectIds: string[]) =>
    invoke<void>('set_paper_projects', { citekey, projectIds }),
};

/**
 * Combined API object
 */
export const api = {
  vault,
  papers,
  importExport,
  pdf,
  claude,
  notes,
  graph,
  jobs,
  settings,
  projects,
};

export default api;
