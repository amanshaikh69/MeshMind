import React from 'react';
import { Brain, Users, Server, LogOut } from 'lucide-react';

interface NavbarProps {
  peerCount: number;
  isLLMHost: boolean;
  onNavigatePeers: () => void;
  onNavigateAnalytics?: () => void;
  username?: string;
  onLogout?: () => void;
}

export function Navbar({ peerCount, isLLMHost, onNavigatePeers, onNavigateAnalytics, username, onLogout }: NavbarProps) {
  return (
    <div className="fixed top-0 left-0 right-0 z-50 bg-surface-2/75 backdrop-blur-md border-b border-divider">
      <div className="max-w-7xl mx-auto px-4 h-16 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Brain className="h-7 w-7 text-accent" />
          <h1 className="text-2xl font-semibold text-bright">MeshMind</h1>
        </div>
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-1 text-sm text-accent">
            <Users className="h-4 w-4" />
            <span>{peerCount} peers</span>
          </div>
          <div className="flex items-center gap-1 text-sm">
            <Server className={`h-4 w-4 ${isLLMHost ? 'text-accent' : 'text-dim'}`} />
            <span className="text-bright/90">{isLLMHost ? 'LLM Host' : 'LLM Client'}</span>
          </div>
          <button
            onClick={onNavigatePeers}
            className="px-3 py-1.5 rounded-md text-sm bg-panel hover:bg-panel-hover border border-divider text-bright transition-colors"
            title="Peer Conversations"
          >
            Peer Conversations
          </button>
          {onNavigateAnalytics && (
            <button
              onClick={onNavigateAnalytics}
              className="px-3 py-1.5 rounded-md text-sm bg-panel hover:bg-panel-hover border border-divider text-bright transition-colors"
              title="Analytics Dashboard"
            >
              Analytics
            </button>
          )}
          {username && (
            <div className="flex items-center gap-3 pl-4 ml-4 border-l border-divider text-sm">
              <span className="text-accent font-medium">{username}</span>
              {onLogout && (
                <button
                  onClick={onLogout}
                  className="px-3 py-1.5 rounded-md text-sm bg-panel hover:bg-panel-hover border border-divider text-bright transition-colors flex items-center gap-1"
                  title="Logout"
                >
                  <LogOut className="h-4 w-4" />
                  Logout
                </button>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}


