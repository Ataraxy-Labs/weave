/**
 * MCP tool for retrieving error state from Next.js dev server.
 *
 * This tool provides comprehensive error reporting including:
 * - Next.js global errors (e.g., next.config validation errors)
 * - Browser runtime errors with source-mapped stack traces
 * - Build errors from webpack/turbopack compilation
 *
 * For browser errors, it leverages the HMR infrastructure for server-to-browser communication.
 *
 * Flow:
 *   MCP client → server generates request ID → HMR message to browser →
 *   browser queries error overlay state → HMR response back → server performs source mapping →
 *   combined with global errors → formatted output.
 */
import type { McpServer } from 'next/dist/compiled/@modelcontextprotocol/sdk/server/mcp';
import type { OverlayState } from '../../../next-devtools/dev-overlay/shared';
import { type HmrMessageSentToBrowser } from '../../dev/hot-reloader-types';
export declare function registerGetErrorsTool(server: McpServer, sendHmrMessage: (message: HmrMessageSentToBrowser) => void, getActiveConnectionCount: () => number): void;
export declare function handleErrorStateResponse(requestId: string, errorState: OverlayState | null, url: string | undefined): void;
