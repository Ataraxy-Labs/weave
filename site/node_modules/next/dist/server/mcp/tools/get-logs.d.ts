/**
 * MCP tool for getting the path to the Next.js development log file.
 *
 * This tool returns the path to the {nextConfig.distDir}/logs/next-development.log file
 * that contains browser console logs and other development information.
 */
import type { McpServer } from 'next/dist/compiled/@modelcontextprotocol/sdk/server/mcp';
export declare function registerGetLogsTool(server: McpServer, distDir: string): void;
