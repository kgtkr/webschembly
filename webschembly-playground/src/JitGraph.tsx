import React, { useMemo } from 'react';
import {
    ReactFlow,
    Controls,
    Background,
    useNodesState,
    useEdgesState,
    Position,
    type Edge,
    type Node,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import dagre from 'dagre';

export type JitLogEvent = {
    type: "bb";
    module_id: number,
    func_id: number;
    env_index: number;
    func_index: number;
    bb_id: number;
    index: number;
    successors: number[];
};

type JitGraphProps = {
    logs: JitLogEvent[];
};

const dagreGraph = new dagre.graphlib.Graph();
dagreGraph.setDefaultEdgeLabel(() => ({}));

const nodeWidth = 250;
const nodeHeight = 50;

const getLayoutedElements = (nodes: Node[], edges: Edge[], direction = 'TB') => {
    dagreGraph.setGraph({ rankdir: direction });

    nodes.forEach((node) => {
        dagreGraph.setNode(node.id, { width: nodeWidth, height: nodeHeight });
    });

    edges.forEach((edge) => {
        dagreGraph.setEdge(edge.source, edge.target);
    });

    dagre.layout(dagreGraph);

    const layoutedNodes: Node[] = nodes.map((node) => {
        const nodeWithPosition = dagreGraph.node(node.id);
        return {
            ...node,
            targetPosition: Position.Top,
            sourcePosition: Position.Bottom,
            position: {
                x: nodeWithPosition.x - nodeWidth / 2,
                y: nodeWithPosition.y - nodeHeight / 2,
            },
        };
    });

    return { nodes: layoutedNodes, edges };
};

export function JitGraph({ logs }: JitGraphProps) {
    const { nodes: initialNodes, edges: initialEdges } = useMemo(() => {
        const nodes: Node[] = [];
        const edges: Edge[] = [];

        const existingNodes = new Set<string>();
        const existingEdges = new Set<string>();

        for (const log of logs) {
            if (log.type === "bb") {
                const nodeId = `bb-${log.module_id}-${log.func_id}-${log.env_index}-${log.func_index}-${log.bb_id}-${log.index}`;
                if (!existingNodes.has(nodeId)) {
                    existingNodes.add(nodeId);
                    nodes.push({
                        id: nodeId,
                        position: { x: 0, y: 0 },
                        data: { label: `module:${log.module_id} func:${log.func_id} env:${log.env_index} c:${log.func_index} bb:${log.bb_id} i:${log.index}` },
                        style: { border: '1px solid #777', padding: '10px', borderRadius: '5px', background: '#fff', color: '#1e293b' }
                    });
                }

                for (const succ of log.successors) {
                    const targetId = `bb-${log.module_id}-${log.func_id}-${log.env_index}-${log.func_index}-${succ}-0`; // Assuming successor index 0 for simplicity if not provided. In our current implementation, successors are just bb_ids, the index isn't passed for successors directly in the log yet, but let's just draw an edge to the base bb for now or assuming default index.
                    const edgeId = `${nodeId}->${targetId}`;
                    if (!existingEdges.has(edgeId)) {
                        existingEdges.add(edgeId);
                        edges.push({
                            id: edgeId,
                            source: nodeId,
                            target: targetId,
                            animated: true,
                        });
                    }
                }
            }
        }

        return getLayoutedElements(nodes, edges);
    }, [logs]);

    return (
        <div style={{ width: '100%', height: '100%' }}>
            <ReactFlow
                nodes={initialNodes}
                edges={initialEdges}
                nodesDraggable={false}
                nodesConnectable={false}
                elementsSelectable={false}
                fitView
            >
                <Background />
                <Controls />
            </ReactFlow>
        </div>
    );
}
