import { Box, Image, Clapperboard, PenLine } from 'lucide-react';
import type { ToolId } from './client';

// Console is a room with independent tools, not a set of Learn tabs.
export const CONSOLE_TOOLS = [
  { id: 'write', title: 'Write', key: '1', Icon: PenLine },
  { id: 'image', title: 'Image', key: '2', Icon: Image },
  { id: 'audiovisual', title: 'Audiovisual', key: '3', Icon: Clapperboard },
  { id: 'three', title: '3D', key: '4', Icon: Box },
] as const;
export const toolById = (id: ToolId) => CONSOLE_TOOLS.find(t => t.id === id)!;
export interface ConsoleContext { tool: ToolId; documentId: string | null; selection: unknown }
export const CONSOLE_CONTEXT_EVENT = 'wi-console-context';
