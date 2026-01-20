/**
 * Network graph view helpers
 *
 * Configuration and utilities for the vis.js network graph.
 */

import type { GraphData, GraphNode, GraphEdge, PaperStatus } from '../types';

/**
 * Node colors by paper status
 */
export const statusColors: Record<PaperStatus, { background: string; border: string }> = {
  discovered: { background: '#E8E8E8', border: '#8B949E' },
  wanted: { background: '#C5D5EA', border: '#3B5998' },
  queued: { background: '#F0E6D2', border: '#B08D57' },
  downloaded: { background: '#D4EDDA', border: '#2D6A4F' },
  summarized: { background: '#E2D9F3', border: '#5B4B8A' },
  failed: { background: '#F8D7DA', border: '#9B2C2C' },
};

/**
 * vis.js network options
 */
export const networkOptions = {
  nodes: {
    shape: 'box',
    font: {
      face: 'Inter, -apple-system, sans-serif',
      size: 12,
      color: '#1F2328',
    },
    borderWidth: 2,
    margin: 10,
    shadow: {
      enabled: true,
      color: 'rgba(0,0,0,0.1)',
      size: 4,
      x: 0,
      y: 2,
    },
  },
  edges: {
    arrows: {
      to: { enabled: true, scaleFactor: 0.5 },
    },
    color: {
      color: '#D4C9B8',
      highlight: '#22324A',
    },
    width: 1.5,
    smooth: {
      type: 'cubicBezier',
      forceDirection: 'horizontal',
      roundness: 0.4,
    },
  },
  physics: {
    enabled: true,
    solver: 'forceAtlas2Based',
    forceAtlas2Based: {
      gravitationalConstant: -50,
      centralGravity: 0.01,
      springLength: 150,
      springConstant: 0.08,
      damping: 0.4,
    },
    stabilization: {
      enabled: true,
      iterations: 200,
      updateInterval: 25,
    },
  },
  interaction: {
    hover: true,
    tooltipDelay: 200,
    zoomView: true,
    dragView: true,
  },
  layout: {
    improvedLayout: true,
  },
};

/**
 * Apply status colors to nodes
 */
export function applyStatusColors(nodes: GraphNode[], statusMap: Record<string, PaperStatus>): GraphNode[] {
  return nodes.map(node => {
    const status = statusMap[node.id] ?? 'discovered';
    const colors = statusColors[status];
    return {
      ...node,
      color: colors,
      font: { color: '#1F2328' },
    };
  });
}

/**
 * Filter graph to show only connected nodes
 */
export function filterConnectedNodes(data: GraphData): GraphData {
  const connectedIds = new Set<string>();

  for (const edge of data.edges) {
    connectedIds.add(edge.from);
    connectedIds.add(edge.to);
  }

  return {
    nodes: data.nodes.filter(node => connectedIds.has(node.id)),
    edges: data.edges,
  };
}

/**
 * Get subgraph centered on a specific node
 */
export function getSubgraph(data: GraphData, centerId: string, depth = 2): GraphData {
  const includedIds = new Set<string>([centerId]);
  const includedEdges: GraphEdge[] = [];

  // BFS to find connected nodes up to depth
  let frontier = new Set<string>([centerId]);

  for (let d = 0; d < depth; d++) {
    const nextFrontier = new Set<string>();

    for (const edge of data.edges) {
      if (frontier.has(edge.from) && !includedIds.has(edge.to)) {
        nextFrontier.add(edge.to);
        includedIds.add(edge.to);
        includedEdges.push(edge);
      }
      if (frontier.has(edge.to) && !includedIds.has(edge.from)) {
        nextFrontier.add(edge.from);
        includedIds.add(edge.from);
        includedEdges.push(edge);
      }
    }

    frontier = nextFrontier;
  }

  // Also include edges between included nodes
  for (const edge of data.edges) {
    if (includedIds.has(edge.from) && includedIds.has(edge.to)) {
      if (!includedEdges.includes(edge)) {
        includedEdges.push(edge);
      }
    }
  }

  return {
    nodes: data.nodes.filter(node => includedIds.has(node.id)),
    edges: includedEdges,
  };
}

/**
 * Calculate graph statistics
 */
export interface GraphStats {
  nodeCount: number;
  edgeCount: number;
  avgDegree: number;
  isolatedNodes: number;
}

export function calculateGraphStats(data: GraphData): GraphStats {
  const degrees = new Map<string, number>();

  for (const node of data.nodes) {
    degrees.set(node.id, 0);
  }

  for (const edge of data.edges) {
    degrees.set(edge.from, (degrees.get(edge.from) ?? 0) + 1);
    degrees.set(edge.to, (degrees.get(edge.to) ?? 0) + 1);
  }

  let totalDegree = 0;
  let isolatedNodes = 0;

  for (const degree of degrees.values()) {
    totalDegree += degree;
    if (degree === 0) isolatedNodes++;
  }

  return {
    nodeCount: data.nodes.length,
    edgeCount: data.edges.length,
    avgDegree: data.nodes.length > 0 ? totalDegree / data.nodes.length : 0,
    isolatedNodes,
  };
}
