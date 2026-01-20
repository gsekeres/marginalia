/**
 * Library view helpers
 *
 * Utility functions for the library view (paper list).
 */

import type { Paper, PaperStatus } from '../types';

/**
 * Status display configuration
 */
export const statusConfig: Record<PaperStatus, { label: string; color: string; icon: string }> = {
  discovered: {
    label: 'Discovered',
    color: 'var(--status-discovered)',
    icon: '○',
  },
  wanted: {
    label: 'Wanted',
    color: 'var(--status-wanted)',
    icon: '★',
  },
  queued: {
    label: 'Queued',
    color: 'var(--status-queued)',
    icon: '◷',
  },
  downloaded: {
    label: 'Downloaded',
    color: 'var(--status-downloaded)',
    icon: '↓',
  },
  summarized: {
    label: 'Summarized',
    color: 'var(--status-summarized)',
    icon: '✓',
  },
  failed: {
    label: 'Failed',
    color: 'var(--status-failed)',
    icon: '✗',
  },
};

/**
 * Format authors for display
 */
export function formatAuthors(authors: string[], maxAuthors = 3): string {
  if (authors.length === 0) return 'Unknown author';
  if (authors.length <= maxAuthors) {
    return authors.join(', ');
  }
  return `${authors.slice(0, maxAuthors).join(', ')} et al.`;
}

/**
 * Format year for display
 */
export function formatYear(year: number | null): string {
  return year?.toString() ?? 'n.d.';
}

/**
 * Get citation string (Author, Year)
 */
export function getCitation(paper: Paper): string {
  const firstAuthor = paper.authors[0]?.split(',')[0] ?? 'Unknown';
  return `${firstAuthor}, ${formatYear(paper.year)}`;
}

/**
 * Sort papers by various criteria
 */
export type SortField = 'title' | 'year' | 'added_at' | 'status';
export type SortOrder = 'asc' | 'desc';

export function sortPapers(papers: Paper[], field: SortField, order: SortOrder): Paper[] {
  const sorted = [...papers].sort((a, b) => {
    let comparison = 0;
    switch (field) {
      case 'title':
        comparison = a.title.localeCompare(b.title);
        break;
      case 'year':
        comparison = (a.year ?? 0) - (b.year ?? 0);
        break;
      case 'added_at':
        comparison = a.added_at.localeCompare(b.added_at);
        break;
      case 'status':
        comparison = a.status.localeCompare(b.status);
        break;
    }
    return order === 'asc' ? comparison : -comparison;
  });
  return sorted;
}

/**
 * Group papers by status
 */
export function groupByStatus(papers: Paper[]): Record<PaperStatus, Paper[]> {
  const groups: Record<PaperStatus, Paper[]> = {
    discovered: [],
    wanted: [],
    queued: [],
    downloaded: [],
    summarized: [],
    failed: [],
  };

  for (const paper of papers) {
    groups[paper.status].push(paper);
  }

  return groups;
}
