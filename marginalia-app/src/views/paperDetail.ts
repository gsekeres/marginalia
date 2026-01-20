/**
 * Paper detail view helpers
 *
 * Utility functions for the paper detail panel.
 */

import type { Paper, RelatedPaper } from '../types';

/**
 * Format a date string for display
 */
export function formatDate(dateStr: string | null): string {
  if (!dateStr) return '';
  try {
    const date = new Date(dateStr);
    return date.toLocaleDateString('en-US', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    });
  } catch {
    return dateStr;
  }
}

/**
 * Format relative time (e.g., "2 days ago")
 */
export function formatRelativeTime(dateStr: string | null): string {
  if (!dateStr) return '';
  try {
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

    if (diffDays === 0) return 'Today';
    if (diffDays === 1) return 'Yesterday';
    if (diffDays < 7) return `${diffDays} days ago`;
    if (diffDays < 30) return `${Math.floor(diffDays / 7)} weeks ago`;
    if (diffDays < 365) return `${Math.floor(diffDays / 30)} months ago`;
    return `${Math.floor(diffDays / 365)} years ago`;
  } catch {
    return '';
  }
}

/**
 * Get journal citation string
 */
export function getJournalCitation(paper: Paper): string {
  const parts: string[] = [];

  if (paper.journal) {
    parts.push(paper.journal);
  }

  if (paper.volume) {
    let vol = paper.volume;
    if (paper.number) {
      vol += `(${paper.number})`;
    }
    parts.push(vol);
  }

  if (paper.pages) {
    parts.push(paper.pages);
  }

  return parts.join(', ');
}

/**
 * Get DOI URL
 */
export function getDoiUrl(doi: string | null): string | null {
  if (!doi) return null;
  if (doi.startsWith('http')) return doi;
  return `https://doi.org/${doi}`;
}

/**
 * Check if paper has PDF
 */
export function hasPdf(paper: Paper): boolean {
  return paper.pdf_path !== null;
}

/**
 * Check if paper has summary
 */
export function hasSummary(paper: Paper): boolean {
  return paper.summary_path !== null;
}

/**
 * Get action buttons for paper based on status
 */
export interface PaperAction {
  label: string;
  action: string;
  icon: string;
  primary?: boolean;
  disabled?: boolean;
}

export function getPaperActions(paper: Paper): PaperAction[] {
  const actions: PaperAction[] = [];

  switch (paper.status) {
    case 'discovered':
      actions.push({
        label: 'Mark Wanted',
        action: 'want',
        icon: '★',
        primary: true,
      });
      break;

    case 'wanted':
      actions.push({
        label: 'Find PDF',
        action: 'find',
        icon: '↓',
        primary: true,
      });
      break;

    case 'downloaded':
      actions.push({
        label: 'Summarize',
        action: 'summarize',
        icon: '✎',
        primary: true,
      });
      actions.push({
        label: 'View PDF',
        action: 'view-pdf',
        icon: '📄',
      });
      break;

    case 'summarized':
      actions.push({
        label: 'View Summary',
        action: 'view-summary',
        icon: '📝',
        primary: true,
      });
      actions.push({
        label: 'View PDF',
        action: 'view-pdf',
        icon: '📄',
      });
      actions.push({
        label: 'Re-summarize',
        action: 'summarize',
        icon: '↻',
      });
      break;

    case 'failed':
      actions.push({
        label: 'Retry',
        action: 'find',
        icon: '↻',
        primary: true,
      });
      actions.push({
        label: 'Manual Links',
        action: 'manual',
        icon: '🔗',
      });
      break;
  }

  return actions;
}

/**
 * Format related paper for display
 */
export function formatRelatedPaper(related: RelatedPaper): string {
  const parts = [related.title];

  if (related.authors.length > 0) {
    const firstAuthor = related.authors[0].split(',')[0];
    parts.push(`(${firstAuthor}${related.year ? `, ${related.year}` : ''})`);
  } else if (related.year) {
    parts.push(`(${related.year})`);
  }

  return parts.join(' ');
}
