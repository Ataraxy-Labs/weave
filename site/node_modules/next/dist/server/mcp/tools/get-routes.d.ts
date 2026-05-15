/**
 * MCP tool for getting all routes that become entry points in a Next.js application.
 *
 * This tool discovers routes by scanning the filesystem directly. It finds all route
 * files in the app/ and pages/ directories and converts them to route paths.
 *
 * Returns routes grouped by router type:
 * - appRouter: App Router pages and route handlers
 * - pagesRouter: Pages Router pages and API routes
 *
 * Dynamic route segments appear as [id], [slug], or [...slug] patterns. This tool
 * does NOT expand getStaticParams - it only shows the route patterns as defined in
 * the filesystem.
 */
import type { McpServer } from 'next/dist/compiled/@modelcontextprotocol/sdk/server/mcp';
import type { NextConfigComplete } from '../../../server/config-shared';
export declare function registerGetRoutesTool(server: McpServer, options: {
    projectPath: string;
    nextConfig: NextConfigComplete;
    pagesDir: string | undefined;
    appDir: string | undefined;
}): void;
