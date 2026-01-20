/**
 * Project state management
 *
 * Handles project CRUD and paper assignments.
 */

import { api } from '../api/client';
import type { Project, CreateProjectRequest, UpdateProjectRequest } from '../types';

export interface ProjectsState {
  projects: Project[];
  selectedProjectId: string | null;
  isLoading: boolean;
  error: string | null;
}

export function createProjectsState(): ProjectsState & {
  load(): Promise<void>;
  create(request: CreateProjectRequest): Promise<Project | null>;
  update(request: UpdateProjectRequest): Promise<boolean>;
  delete(id: string): Promise<boolean>;
  select(id: string | null): void;
  addPaper(projectId: string, citekey: string): Promise<boolean>;
  removePaper(projectId: string, citekey: string): Promise<boolean>;
  getPaperProjects(citekey: string): Promise<Project[]>;
  setPaperProjects(citekey: string, projectIds: string[]): Promise<boolean>;
  getSelectedProject(): Project | null;
} {
  return {
    // State
    projects: [],
    selectedProjectId: null,
    isLoading: false,
    error: null,

    // Load all projects
    async load() {
      this.isLoading = true;
      this.error = null;

      try {
        this.projects = await api.projects.list();
      } catch (e) {
        this.error = e instanceof Error ? e.message : String(e);
        console.error('Failed to load projects:', e);
      } finally {
        this.isLoading = false;
      }
    },

    // Create a new project
    async create(request: CreateProjectRequest) {
      try {
        const project = await api.projects.create(request);
        this.projects.push(project);
        // Sort by name
        this.projects.sort((a, b) => a.name.localeCompare(b.name));
        return project;
      } catch (e) {
        this.error = e instanceof Error ? e.message : String(e);
        console.error('Failed to create project:', e);
        return null;
      }
    },

    // Update a project
    async update(request: UpdateProjectRequest) {
      try {
        await api.projects.update(request);
        // Update local state
        const index = this.projects.findIndex(p => p.id === request.id);
        if (index !== -1) {
          this.projects[index] = {
            ...this.projects[index],
            name: request.name,
            color: request.color,
            description: request.description ?? null,
          };
          // Re-sort by name
          this.projects.sort((a, b) => a.name.localeCompare(b.name));
        }
        return true;
      } catch (e) {
        this.error = e instanceof Error ? e.message : String(e);
        console.error('Failed to update project:', e);
        return false;
      }
    },

    // Delete a project
    async delete(id: string) {
      try {
        await api.projects.delete(id);
        this.projects = this.projects.filter(p => p.id !== id);
        if (this.selectedProjectId === id) {
          this.selectedProjectId = null;
        }
        return true;
      } catch (e) {
        this.error = e instanceof Error ? e.message : String(e);
        console.error('Failed to delete project:', e);
        return false;
      }
    },

    // Select a project (or null for "All Papers")
    select(id: string | null) {
      this.selectedProjectId = id;
    },

    // Add a paper to a project
    async addPaper(projectId: string, citekey: string) {
      try {
        await api.projects.addPaper(projectId, citekey);
        // Update paper count
        const project = this.projects.find(p => p.id === projectId);
        if (project) {
          project.paper_count++;
        }
        return true;
      } catch (e) {
        this.error = e instanceof Error ? e.message : String(e);
        console.error('Failed to add paper to project:', e);
        return false;
      }
    },

    // Remove a paper from a project
    async removePaper(projectId: string, citekey: string) {
      try {
        await api.projects.removePaper(projectId, citekey);
        // Update paper count
        const project = this.projects.find(p => p.id === projectId);
        if (project && project.paper_count > 0) {
          project.paper_count--;
        }
        return true;
      } catch (e) {
        this.error = e instanceof Error ? e.message : String(e);
        console.error('Failed to remove paper from project:', e);
        return false;
      }
    },

    // Get projects a paper belongs to
    async getPaperProjects(citekey: string) {
      try {
        return await api.projects.getPaperProjects(citekey);
      } catch (e) {
        console.error('Failed to get paper projects:', e);
        return [];
      }
    },

    // Set projects for a paper
    async setPaperProjects(citekey: string, projectIds: string[]) {
      try {
        await api.projects.setPaperProjects(citekey, projectIds);
        // Reload projects to update paper counts
        await this.load();
        return true;
      } catch (e) {
        this.error = e instanceof Error ? e.message : String(e);
        console.error('Failed to set paper projects:', e);
        return false;
      }
    },

    // Get the currently selected project
    getSelectedProject() {
      if (!this.selectedProjectId) return null;
      return this.projects.find(p => p.id === this.selectedProjectId) ?? null;
    },
  };
}

export type ProjectsStore = ReturnType<typeof createProjectsState>;
