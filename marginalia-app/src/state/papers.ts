/**
 * Papers state management
 *
 * Handles paper listing, filtering, and selection.
 */

import { api } from '../api/client';
import type { Paper, PaperStatus, PaperQuery } from '../types';

export interface PapersState {
  items: Paper[];
  selected: Paper | null;
  filter: {
    status: PaperStatus | null;
    search: string;
  };
  isLoading: boolean;
  error: string | null;
}

export function createPapersState(getVaultPath: () => string | null): PapersState & {
  load(options?: PaperQuery): Promise<void>;
  select(citekey: string): Promise<void>;
  clearSelection(): void;
  setFilter(status: PaperStatus | null): void;
  setSearch(query: string): void;
  updateStatus(citekey: string, status: PaperStatus): Promise<boolean>;
  refresh(): Promise<void>;
} {
  return {
    // State
    items: [],
    selected: null,
    filter: {
      status: null,
      search: '',
    },
    isLoading: false,
    error: null,

    // Load papers with optional filtering
    async load(options?: PaperQuery) {
      const vaultPath = getVaultPath();
      if (!vaultPath) return;

      this.isLoading = true;
      this.error = null;

      try {
        const query: PaperQuery = {
          ...options,
          status: this.filter.status ?? undefined,
          search: this.filter.search || undefined,
        };
        this.items = await api.papers.list(vaultPath, query);
      } catch (e) {
        this.error = e instanceof Error ? e.message : String(e);
        this.items = [];
      } finally {
        this.isLoading = false;
      }
    },

    // Select a paper by citekey
    async select(citekey: string) {
      const vaultPath = getVaultPath();
      if (!vaultPath) return;

      try {
        this.selected = await api.papers.get(vaultPath, citekey);
      } catch (e) {
        console.error('Failed to select paper:', e);
      }
    },

    // Clear selection
    clearSelection() {
      this.selected = null;
    },

    // Set status filter
    setFilter(status: PaperStatus | null) {
      this.filter.status = status;
      this.load();
    },

    // Set search query
    setSearch(query: string) {
      this.filter.search = query;
      this.load();
    },

    // Update paper status
    async updateStatus(citekey: string, status: PaperStatus) {
      const vaultPath = getVaultPath();
      if (!vaultPath) return false;

      try {
        await api.papers.updateStatus(vaultPath, citekey, status);
        // Refresh the list and selected paper
        await this.load();
        if (this.selected?.citekey === citekey) {
          await this.select(citekey);
        }
        return true;
      } catch (e) {
        console.error('Failed to update paper status:', e);
        return false;
      }
    },

    // Refresh current view
    async refresh() {
      await this.load();
      if (this.selected) {
        await this.select(this.selected.citekey);
      }
    },
  };
}

export type PapersStore = ReturnType<typeof createPapersState>;
