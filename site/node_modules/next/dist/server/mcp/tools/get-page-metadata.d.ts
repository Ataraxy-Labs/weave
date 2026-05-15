import type { McpServer } from 'next/dist/compiled/@modelcontextprotocol/sdk/server/mcp';
import { type HmrMessageSentToBrowser } from '../../dev/hot-reloader-types';
import type { SegmentTrieData } from '../../../shared/lib/mcp-page-metadata-types';
export declare function registerGetPageMetadataTool(server: McpServer, sendHmrMessage: (message: HmrMessageSentToBrowser) => void, getActiveConnectionCount: () => number): void;
export declare function handlePageMetadataResponse(requestId: string, segmentTrieData: SegmentTrieData | null, url: string | undefined): void;
