/**
 * Telemetry tracker for MCP tool call usage.
 * Tracks invocation counts for each MCP tool to be reported via telemetry.
 */
import type { McpToolName } from '../../telemetry/events/build';
export interface McpToolUsage {
    featureName: McpToolName;
    invocationCount: number;
}
declare class McpTelemetryTracker {
    private usageMap;
    /**
     * Record a tool call invocation
     */
    recordToolCall(toolName: McpToolName): void;
    /**
     * Get all tool usages as an array
     */
    getUsages(): McpToolUsage[];
    /**
     * Reset all usage tracking
     */
    reset(): void;
    /**
     * Check if any tools have been called
     */
    hasUsage(): boolean;
}
export declare const mcpTelemetryTracker: McpTelemetryTracker;
/**
 * Get MCP tool usage telemetry
 */
export declare function getMcpTelemetryUsage(): McpToolUsage[];
/**
 * Reset MCP telemetry tracker
 */
export declare function resetMcpTelemetry(): void;
/**
 * Record MCP telemetry usage to the telemetry instance
 */
export declare function recordMcpTelemetry(telemetry: {
    record: (event: any) => void;
}): void;
export {};
