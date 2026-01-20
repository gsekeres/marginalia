/**
 * Marginalia Main Entry Point
 *
 * This module initializes the application, sets up Alpine.js,
 * and exposes the API client and helpers to the global scope.
 */

import Alpine from 'alpinejs';
import api from './api/client';
import * as state from './state';
import * as views from './views';
import * as components from './components';

// Expose modules to global scope for use in Alpine templates
declare global {
  interface Window {
    Alpine: typeof Alpine;
    api: typeof api;
    state: typeof state;
    views: typeof views;
    components: typeof components;
  }
}

window.api = api;
window.state = state;
window.views = views;
window.components = components;

// Initialize Alpine
window.Alpine = Alpine;

// Start Alpine when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
  Alpine.start();
});

// Re-export everything for module usage
export * from './types';
export { api };
export { state };
export { views };
export { components };
