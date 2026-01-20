/**
 * Job progress component
 *
 * Display and track background job progress.
 */

import type { Job, JobStatus, JobType, JobUpdate } from '../types';

/**
 * Job type display configuration
 */
export const jobTypeConfig: Record<JobType, { label: string; icon: string }> = {
  import_bib: { label: 'Importing BibTeX', icon: '📚' },
  find_pdf: { label: 'Finding PDF', icon: '🔍' },
  download_pdf: { label: 'Downloading PDF', icon: '↓' },
  extract_text: { label: 'Extracting Text', icon: '📄' },
  summarize: { label: 'Summarizing', icon: '✎' },
  build_graph: { label: 'Building Graph', icon: '🕸' },
};

/**
 * Job status display configuration
 */
export const jobStatusConfig: Record<JobStatus, { label: string; color: string }> = {
  pending: { label: 'Pending', color: '#8B949E' },
  running: { label: 'Running', color: '#3B5998' },
  completed: { label: 'Completed', color: '#2D6A4F' },
  failed: { label: 'Failed', color: '#9B2C2C' },
  cancelled: { label: 'Cancelled', color: '#B08D57' },
};

/**
 * Format job for display
 */
export function formatJob(job: Job): {
  typeLabel: string;
  typeIcon: string;
  statusLabel: string;
  statusColor: string;
  description: string;
} {
  const typeInfo = jobTypeConfig[job.job_type] ?? { label: job.job_type, icon: '⚙' };
  const statusInfo = jobStatusConfig[job.status];

  let description = typeInfo.label;
  if (job.citekey) {
    description += `: ${job.citekey}`;
  }

  return {
    typeLabel: typeInfo.label,
    typeIcon: typeInfo.icon,
    statusLabel: statusInfo.label,
    statusColor: statusInfo.color,
    description,
  };
}

/**
 * Calculate estimated time remaining (very rough)
 */
export function estimateTimeRemaining(job: Job): string | null {
  if (job.status !== 'running' || job.progress <= 0) {
    return null;
  }

  if (!job.started_at) {
    return null;
  }

  try {
    const startTime = new Date(job.started_at).getTime();
    const now = Date.now();
    const elapsed = now - startTime;
    const estimatedTotal = elapsed / (job.progress / 100);
    const remaining = estimatedTotal - elapsed;

    if (remaining < 0) return null;

    const seconds = Math.round(remaining / 1000);
    if (seconds < 60) return `~${seconds}s remaining`;
    const minutes = Math.round(seconds / 60);
    return `~${minutes}m remaining`;
  } catch {
    return null;
  }
}

/**
 * Check if job is active
 */
export function isActiveJob(job: Job): boolean {
  return job.status === 'pending' || job.status === 'running';
}

/**
 * Check if job can be cancelled
 */
export function canCancelJob(job: Job): boolean {
  return isActiveJob(job);
}

/**
 * Create a job progress tracker for Alpine.js
 */
export function createJobProgress(onUpdate?: (update: JobUpdate) => void) {
  return {
    // Track jobs by ID for quick updates
    jobsById: new Map<string, Job>(),

    // Update a job from an event
    handleUpdate(update: JobUpdate) {
      const existing = this.jobsById.get(update.id);
      if (existing) {
        existing.status = update.status as JobStatus;
        existing.progress = update.progress;
        existing.error = update.error;
      }
      onUpdate?.(update);
    },

    // Add a job to track
    trackJob(job: Job) {
      this.jobsById.set(job.id, job);
    },

    // Remove a job from tracking
    untrackJob(id: string) {
      this.jobsById.delete(id);
    },

    // Get formatted job info
    getJobInfo(id: string) {
      const job = this.jobsById.get(id);
      if (!job) return null;
      return formatJob(job);
    },
  };
}
