import React, { useState, useEffect } from 'react';
import { getAllSharedFiles, FileInfo, API_BASE_URL, getPeerConversations } from '../api/llm';
import { Download, Image, ChevronDown, ChevronUp, User, Users, MessageSquare } from 'lucide-react';

export function SharedFilesPanel() {
  const [files, setFiles] = useState<FileInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [isCollapsed, setIsCollapsed] = useState(false);
  const [localIp, setLocalIp] = useState<string | null>(null);
  const [ipToName, setIpToName] = useState<Record<string, string>>({});

  useEffect(() => {
    // Get local IP from API
    fetch(`${API_BASE_URL}/api/local`)
      .then(res => res.json())
      .then(data => {
        if (data?.host_info?.ip_address) {
          setLocalIp(data.host_info.ip_address);
        }
      })
      .catch(() => {}); // Ignore errors

    async function fetchFiles() {
      setLoading(true);
      setError(null);
      try {
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), 5000); // 5 second timeout
        
        const result = await getAllSharedFiles();
        clearTimeout(timeoutId);
        setFiles(result);
      } catch (e: any) {
        if (e.name === 'AbortError') {
          setError('Request timeout');
        } else {
          setError(e.message || 'Failed to fetch files');
        }
      }
      setLoading(false);
    }

    async function fetchPeers() {
      try {
        const peers = await getPeerConversations();
        const map: Record<string, string> = {};
        Object.entries(peers).forEach(([ip, conv]: any) => {
          const name = conv?.host_info?.hostname;
          if (name && typeof name === 'string' && name.trim().length > 0) {
            map[ip] = name;
          }
        });
        setIpToName(map);
      } catch {}
    }
    
    // Initial fetch
    fetchFiles();
    fetchPeers();
    
    // Refresh every 5 seconds
    const intervalFiles = setInterval(fetchFiles, 5000);
    const intervalPeers = setInterval(fetchPeers, 10000);
    return () => { clearInterval(intervalFiles); clearInterval(intervalPeers); };
  }, []);

  if (error && files.length === 0) return <div className="p-4 text-red-500">{error}</div>;
  if (files.length === 0 && loading) return <div className="p-4 text-center text-gray-400">Loading shared files...</div>;

  // Separate files by source
  const myFiles = localIp ? files.filter(f => f.uploader_ip === localIp || f.uploader_ip === '127.0.0.1') : [];
  const peerFiles = localIp ? files.filter(f => f.uploader_ip !== localIp && f.uploader_ip !== '127.0.0.1') : files;

  return (
    <div className="w-full mb-3">
      <div 
        className="bg-panel border border-divider rounded-lg shadow-soft overflow-hidden"
        style={{ maxHeight: isCollapsed ? '60px' : '400px', transition: 'max-height 0.3s ease' }}
      >
        <button
          onClick={() => setIsCollapsed(!isCollapsed)}
          className="w-full flex items-center justify-between p-4 bg-surface/40 border-b border-divider hover:bg-surface/60 transition-colors"
        >
          <h2 className="text-lg font-semibold text-accent flex items-center gap-2">
            <Users className="w-5 h-5" />
            Shared Files ({files.length})
          </h2>
          {isCollapsed ? (
            <ChevronDown className="w-5 h-5 text-dim" />
          ) : (
            <ChevronUp className="w-5 h-5 text-dim" />
          )}
        </button>

        {!isCollapsed && (
          <div className="p-4 overflow-y-auto overflow-x-hidden" style={{ maxHeight: '340px' }}>
            {files.length === 0 ? (
              <div className="text-dim text-center py-4">No files shared yet.</div>
            ) : (
              <div className="space-y-4">
                {peerFiles.length > 0 && (
                  <div>
                    <h3 className="text-sm font-semibold text-accent mb-2 flex items-center gap-2">
                      <Users className="w-4 h-4" />
                      From Peers ({peerFiles.length})
                    </h3>
                    <ul className="space-y-2">
                      {peerFiles.map(file => (
                        <li key={file.filename + file.upload_time} className="flex items-center justify-between gap-3 bg-surface border border-divider rounded-lg px-3 py-2 hover:bg-surface/80 transition-colors min-w-0">
                          <div className="min-w-0">
                            <div className="font-mono text-sm text-bright truncate" title={file.filename}>{file.filename}</div>
                            <div className="text-[11px] text-dim mt-0.5 flex items-center gap-2">
                              <span>{(file.file_size / 1024).toFixed(1)} KB</span>
                              <span className="hidden md:inline-flex items-center gap-1 text-accent">
                                <User className="w-3 h-3" /> {ipToName[file.uploader_ip] || file.uploader_ip}
                              </span>
                            </div>
                          </div>
                          <div className="flex items-center gap-2 flex-shrink-0">
                            <button
                              onClick={() => window.dispatchEvent(new CustomEvent('meshmind:ask-file', { detail: { filename: file.filename } }))}
                              className="h-7 w-7 inline-flex items-center justify-center bg-panel border border-divider rounded hover:bg-panel/80"
                              title="Ask about this file"
                            >
                              <MessageSquare className="w-4 h-4 text-accent" />
                            </button>
                            {(() => { 
                              const isLocal = file.uploader_ip === localIp || file.uploader_ip === '127.0.0.1';
                              const href = isLocal
                                ? `${API_BASE_URL}/api/files/${encodeURIComponent(file.filename)}`
                                : `${API_BASE_URL}/api/peer-file/${file.uploader_ip}/${encodeURIComponent(file.filename)}`;
                              return (
                                <a
                                  href={href}
                                  download={file.filename}
                                  className="h-7 w-7 inline-flex items-center justify-center bg-accent text-black rounded hover:bg-accent/90"
                                  title="Download"
                                >
                                  <Download className="w-4 h-4" />
                                </a>
                              );
                            })()}
                            {file.file_type.startsWith('image/') && (() => { 
                              const isLocal = file.uploader_ip === localIp || file.uploader_ip === '127.0.0.1';
                              const href = isLocal
                                ? `${API_BASE_URL}/api/files/${encodeURIComponent(file.filename)}`
                                : `${API_BASE_URL}/api/peer-file/${file.uploader_ip}/${encodeURIComponent(file.filename)}`;
                              return (
                                <a
                                  href={href}
                                  target="_blank"
                                  rel="noopener noreferrer"
                                  className="h-7 w-7 inline-flex items-center justify-center bg-bright text-black rounded hover:bg-accent/90"
                                  title="Preview image"
                                >
                                  <Image className="w-4 h-4" />
                                </a>
                              ); 
                            })()}
                          </div>
                        </li>
                      ))}
                    </ul>
                  </div>
                )}

                {myFiles.length > 0 && (
                  <div>
                    <h3 className="text-sm font-semibold text-bright mb-2 flex items-center gap-2">
                      <User className="w-4 h-4" />
                      My Files ({myFiles.length})
                    </h3>
                    <ul className="space-y-2">
                      {myFiles.map(file => (
                        <li key={file.filename + file.upload_time} className="flex items-center justify-between gap-3 bg-surface border border-divider rounded-lg px-3 py-2 hover:bg-surface/80 transition-colors min-w-0">
                          <div className="min-w-0">
                            <div className="font-mono text-sm text-bright truncate" title={file.filename}>{file.filename}</div>
                            <div className="text-[11px] text-dim mt-0.5">{(file.file_size / 1024).toFixed(1)} KB • by me</div>
                          </div>
                          <div className="flex items-center gap-2 flex-shrink-0">
                            <button
                              onClick={() => window.dispatchEvent(new CustomEvent('meshmind:ask-file', { detail: { filename: file.filename } }))}
                              className="h-7 w-7 inline-flex items-center justify-center bg-panel border border-divider rounded hover:bg-panel/80"
                              title="Ask about this file"
                            >
                              <MessageSquare className="w-4 h-4 text-accent" />
                            </button>
                            <a
                              href={`${API_BASE_URL}/api/files/${encodeURIComponent(file.filename)}`}
                              download={file.filename}
                              className="h-7 w-7 inline-flex items-center justify-center bg-accent text-black rounded hover:bg-accent/90"
                              title="Download"
                            >
                              <Download className="w-4 h-4" />
                            </a>
                            {file.file_type.startsWith('image/') && (
                              <a
                                href={`${API_BASE_URL}/api/files/${encodeURIComponent(file.filename)}`}
                                target="_blank"
                                rel="noopener noreferrer"
                                className="h-7 w-7 inline-flex items-center justify-center bg-bright text-black rounded hover:bg-accent/90"
                                title="Preview image"
                              >
                                <Image className="w-4 h-4" />
                              </a>
                            )}
                          </div>
                        </li>
                      ))}
                    </ul>
                  </div>
                )}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}