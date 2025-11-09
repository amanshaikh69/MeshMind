import React from 'react';
import { SharedFilesPanel } from './SharedFilesPanel';

export function FileSidebar() {
  return (
    <aside className="w-full bg-surface/70 backdrop-blur-sm">
      <div className="h-16 border-b border-divider flex items-center px-4 text-sm text-bright/80">
        Shared Files
      </div>
      <div className="p-3">
        <SharedFilesPanel />
      </div>
    </aside>
  );
}


