/**
 * Toast notification component
 *
 * Simple toast notification system for Alpine.js.
 */

export type ToastType = 'success' | 'error' | 'info' | 'warning';

export interface Toast {
  id: string;
  type: ToastType;
  message: string;
  duration: number;
}

export interface ToasterState {
  toasts: Toast[];
}

/**
 * Create toaster state for Alpine.js
 */
export function createToaster(): ToasterState & {
  show(message: string, type?: ToastType, duration?: number): void;
  success(message: string): void;
  error(message: string): void;
  info(message: string): void;
  warning(message: string): void;
  dismiss(id: string): void;
} {
  return {
    toasts: [],

    show(message: string, type: ToastType = 'info', duration = 4000) {
      const id = `toast-${Date.now()}-${Math.random().toString(36).slice(2)}`;
      const toast: Toast = { id, type, message, duration };

      this.toasts.push(toast);

      // Auto-dismiss
      if (duration > 0) {
        setTimeout(() => {
          this.dismiss(id);
        }, duration);
      }
    },

    success(message: string) {
      this.show(message, 'success');
    },

    error(message: string) {
      this.show(message, 'error', 6000);
    },

    info(message: string) {
      this.show(message, 'info');
    },

    warning(message: string) {
      this.show(message, 'warning', 5000);
    },

    dismiss(id: string) {
      const index = this.toasts.findIndex(t => t.id === id);
      if (index !== -1) {
        this.toasts.splice(index, 1);
      }
    },
  };
}

/**
 * Toast styling configuration
 */
export const toastStyles: Record<ToastType, { bg: string; border: string; icon: string }> = {
  success: {
    bg: '#D4EDDA',
    border: '#2D6A4F',
    icon: '✓',
  },
  error: {
    bg: '#F8D7DA',
    border: '#9B2C2C',
    icon: '✗',
  },
  info: {
    bg: '#CCE5FF',
    border: '#3B5998',
    icon: 'ℹ',
  },
  warning: {
    bg: '#FFF3CD',
    border: '#B08D57',
    icon: '⚠',
  },
};

export type Toaster = ReturnType<typeof createToaster>;
