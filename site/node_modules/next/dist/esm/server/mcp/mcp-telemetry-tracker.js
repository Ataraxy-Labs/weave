/**
 * Telemetry tracker for MCP tool call usage.
 * Tracks invocation counts for each MCP tool to be reported via telemetry.
 */ class McpTelemetryTracker {
    /**
   * Record a tool call invocation
   */ recordToolCall(toolName) {
        const current = this.usageMap.get(toolName) || 0;
        this.usageMap.set(toolName, current + 1);
    }
    /**
   * Get all tool usages as an array
   */ getUsages() {
        return Array.from(this.usageMap.entries()).map(([featureName, count])=>({
                featureName,
                invocationCount: count
            }));
    }
    /**
   * Reset all usage tracking
   */ reset() {
        this.usageMap.clear();
    }
    /**
   * Check if any tools have been called
   */ hasUsage() {
        return this.usageMap.size > 0;
    }
    constructor(){
        this.usageMap = new Map();
    }
}
// Singleton instance
export const mcpTelemetryTracker = new McpTelemetryTracker();
/**
 * Get MCP tool usage telemetry
 */ export function getMcpTelemetryUsage() {
    return mcpTelemetryTracker.getUsages();
}
/**
 * Reset MCP telemetry tracker
 */ export function resetMcpTelemetry() {
    mcpTelemetryTracker.reset();
}
/**
 * Record MCP telemetry usage to the telemetry instance
 */ export function recordMcpTelemetry(telemetry) {
    const mcpUsages = getMcpTelemetryUsage();
    if (mcpUsages.length === 0) {
        return;
    }
    const { eventMcpToolUsage } = require('../../telemetry/events/build');
    const events = eventMcpToolUsage(mcpUsages);
    for (const event of events){
        telemetry.record(event);
    }
}

//# sourceMappingURL=mcp-telemetry-tracker.js.map