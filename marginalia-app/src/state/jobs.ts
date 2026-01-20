/**
 * Jobs state management
 *
 * Handles background job tracking and event subscriptions.
 */

import { api } from '../api/client';
import type { Job, JobUpdate, JobType } from '../types';
import type { UnlistenFn } from '@tauri-apps/api/event';

export interface JobsState {
  active: Job[];
  recent: Job[];
  isLoading: boolean;
}

export function createJobsState(): JobsState & {
  init(): Promise<void>;
  startJob(jobType: JobType, citekey?: string): Promise<string | null>;
  cancelJob(jobId: string): Promise<boolean>;
  refresh(): Promise<void>;
  subscribe(callback: (update: JobUpdate) => void): Promise<UnlistenFn>;
} {
  return {
    // State
    active: [],
    recent: [],
    isLoading: false,

    // Initialize - load active jobs
    async init() {
      await this.refresh();
    },

    // Start a new job
    async startJob(jobType: JobType, citekey?: string) {
      try {
        const jobId = await api.jobs.start(jobType, citekey);
        await this.refresh();
        return jobId;
      } catch (e) {
        console.error('Failed to start job:', e);
        return null;
      }
    },

    // Cancel a job
    async cancelJob(jobId: string) {
      try {
        const cancelled = await api.jobs.cancel(jobId);
        if (cancelled) {
          await this.refresh();
        }
        return cancelled;
      } catch (e) {
        console.error('Failed to cancel job:', e);
        return false;
      }
    },

    // Refresh job lists
    async refresh() {
      this.isLoading = true;
      try {
        this.active = await api.jobs.listActive();
        this.recent = await api.jobs.list(undefined, 10);
      } catch (e) {
        console.error('Failed to refresh jobs:', e);
      } finally {
        this.isLoading = false;
      }
    },

    // Subscribe to job updates
    async subscribe(callback: (update: JobUpdate) => void) {
      return api.jobs.onUpdate((update) => {
        callback(update);
        // Auto-refresh on status change
        this.refresh();
      });
    },
  };
}

export type JobsStore = ReturnType<typeof createJobsState>;
