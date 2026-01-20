/**
 * Vault state management
 *
 * Handles vault opening, closing, and statistics.
 */

import { api } from '../api/client';
import type { VaultStats } from '../types';

export interface VaultState {
  path: string | null;
  stats: VaultStats | null;
  recentVaults: string[];
  isLoading: boolean;
  error: string | null;
}

export function createVaultState(): VaultState & {
  init(): Promise<void>;
  open(path: string): Promise<boolean>;
  close(): void;
  refresh(): Promise<void>;
} {
  return {
    // State
    path: null,
    stats: null,
    recentVaults: [],
    isLoading: false,
    error: null,

    // Initialize - load recent vaults
    async init() {
      try {
        this.recentVaults = await api.vault.getRecent();
      } catch (e) {
        console.error('Failed to load recent vaults:', e);
        this.recentVaults = [];
      }
    },

    // Open a vault
    async open(path: string) {
      this.isLoading = true;
      this.error = null;

      try {
        this.stats = await api.vault.open(path);
        this.path = path;
        await api.vault.addRecent(path);
        this.recentVaults = await api.vault.getRecent();
        return true;
      } catch (e) {
        this.error = e instanceof Error ? e.message : String(e);
        return false;
      } finally {
        this.isLoading = false;
      }
    },

    // Close current vault
    close() {
      this.path = null;
      this.stats = null;
      this.error = null;
    },

    // Refresh vault statistics
    async refresh() {
      if (!this.path) return;

      try {
        this.stats = await api.vault.getStats(this.path);
      } catch (e) {
        console.error('Failed to refresh vault stats:', e);
      }
    },
  };
}

export type VaultStore = ReturnType<typeof createVaultState>;
