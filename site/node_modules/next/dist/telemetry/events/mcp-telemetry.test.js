/**
 * @jest-environment node
 */ "use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
const _build = require("./build");
describe('MCP Telemetry Events', ()=>{
    it('should generate correct telemetry events for single tool', ()=>{
        const usages = [
            {
                featureName: 'mcp/get_errors',
                invocationCount: 5
            }
        ];
        const events = (0, _build.eventMcpToolUsage)(usages);
        expect(events).toHaveLength(1);
        expect(events[0]).toEqual({
            eventName: _build.EVENT_MCP_TOOL_USAGE,
            payload: {
                toolName: 'mcp/get_errors',
                invocationCount: 5
            }
        });
    });
    it('should generate correct telemetry events for multiple tools', ()=>{
        const usages = [
            {
                featureName: 'mcp/get_errors',
                invocationCount: 3
            },
            {
                featureName: 'mcp/get_logs',
                invocationCount: 1
            },
            {
                featureName: 'mcp/get_page_metadata',
                invocationCount: 7
            }
        ];
        const events = (0, _build.eventMcpToolUsage)(usages);
        expect(events).toHaveLength(3);
        expect(events[0]).toEqual({
            eventName: _build.EVENT_MCP_TOOL_USAGE,
            payload: {
                toolName: 'mcp/get_errors',
                invocationCount: 3
            }
        });
        expect(events[1]).toEqual({
            eventName: _build.EVENT_MCP_TOOL_USAGE,
            payload: {
                toolName: 'mcp/get_logs',
                invocationCount: 1
            }
        });
        expect(events[2]).toEqual({
            eventName: _build.EVENT_MCP_TOOL_USAGE,
            payload: {
                toolName: 'mcp/get_page_metadata',
                invocationCount: 7
            }
        });
    });
    it('should handle all MCP tool types', ()=>{
        const allTools = [
            'mcp/get_errors',
            'mcp/get_logs',
            'mcp/get_page_metadata',
            'mcp/get_project_metadata',
            'mcp/get_server_action_by_id'
        ];
        const usages = allTools.map((tool, index)=>({
                featureName: tool,
                invocationCount: index + 1
            }));
        const events = (0, _build.eventMcpToolUsage)(usages);
        expect(events).toHaveLength(5);
        events.forEach((event, index)=>{
            expect(event.eventName).toBe(_build.EVENT_MCP_TOOL_USAGE);
            expect(event.payload.toolName).toBe(allTools[index]);
            expect(event.payload.invocationCount).toBe(index + 1);
        });
    });
    it('should handle empty usage array', ()=>{
        const events = (0, _build.eventMcpToolUsage)([]);
        expect(events).toEqual([]);
    });
    it('should use correct event name constant', ()=>{
        expect(_build.EVENT_MCP_TOOL_USAGE).toBe('NEXT_MCP_TOOL_USAGE');
    });
    it('should transform featureName to toolName in payload', ()=>{
        const usages = [
            {
                featureName: 'mcp/get_project_metadata',
                invocationCount: 2
            }
        ];
        const events = (0, _build.eventMcpToolUsage)(usages);
        // Verify the input has 'featureName' but output has 'toolName'
        expect(usages[0]).toHaveProperty('featureName');
        expect(events[0].payload).toHaveProperty('toolName');
        expect(events[0].payload).not.toHaveProperty('featureName');
    });
});

//# sourceMappingURL=mcp-telemetry.test.js.map