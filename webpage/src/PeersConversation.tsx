import React, { useEffect, useState } from 'react';
import { getPeerConversations, Conversation } from './api/llm';
import { Bot } from 'lucide-react';

export function PeersConversation() {
  const [peerConversations, setPeerConversations] = useState<Record<string, Conversation>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedPeer, setSelectedPeer] = useState<string | null>(null);

  useEffect(() => {
    const fetchPeerConversations = async () => {
      try {
        setLoading(true);
        const conversations = await getPeerConversations();
        console.log('Received peer conversations:', conversations); // Debug log
        setPeerConversations(conversations);
        setError(null);

        // If we have conversations and no peer is selected, select the first one
        const peerIps = Object.keys(conversations);
        if (peerIps.length > 0 && !selectedPeer) {
          setSelectedPeer(peerIps[0]);
        }
      } catch (err) {
        console.error('Error loading peer conversations:', err);
        setError('Failed to load peer conversations. Please try again.');
      } finally {
        setLoading(false);
      }
    };

    fetchPeerConversations();
    // Refresh every 5 seconds
    const interval = setInterval(fetchPeerConversations, 5000);
    return () => clearInterval(interval);
  }, []);

  const peerIps = Object.keys(peerConversations);

  return (
    <div className="min-h-screen bg-[#111111] text-white">
      <div className="border-b border-gray-800 p-4">
        <div className="flex items-center justify-center gap-2">
          <Bot className="h-7 w-7 text-blue-400" />
          <h1 className="text-4xl font-bold text-white">Peer Conversations</h1>
        </div>
      </div>

      <div className="max-w-6xl mx-auto p-4 md:p-6">
        {loading && peerIps.length === 0 ? (
          <div className="flex items-center justify-center">
            <div className="animate-spin rounded-full h-8 w-8 border-t-2 border-b-2 border-blue-500"></div>
          </div>
        ) : error ? (
          <div className="text-center text-red-500 bg-red-900/20 rounded-lg p-4">
            {error}
          </div>
        ) : peerIps.length === 0 ? (
          <div className="text-center text-gray-400 mt-8">
            No peer conversations available
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-6">
            {/* IP Address Buttons */}
            <div className="flex flex-wrap gap-4 mb-6">
              {peerIps.map((ip) => (
                <button
                  key={ip}
                  onClick={() => setSelectedPeer(ip)}
                  className={`px-6 py-3 rounded-lg transition-colors ${
                    selectedPeer === ip
                      ? 'bg-blue-500 text-white'
                      : 'bg-[#1a1a1a] hover:bg-[#222222] text-white'
                  }`}
                >
                  {ip}
                </button>
              ))}
            </div>

            {/* Selected Peer's Conversation */}
            {selectedPeer && peerConversations[selectedPeer] && (
              <div className="bg-[#1a1a1a] rounded-lg p-6">
                <div className="mb-4 border-b border-gray-800 pb-4">
                  <h2 className="text-xl font-semibold text-blue-400">Peer: {selectedPeer}</h2>
                  <div className="text-sm text-gray-400 mt-2">
                    <div>Hostname: {peerConversations[selectedPeer].host_info.hostname}</div>
                    <div>LLM Host: {peerConversations[selectedPeer].host_info.is_llm_host ? 'Yes' : 'No'}</div>
                  </div>
                </div>

                <div className="space-y-4">
                  {peerConversations[selectedPeer].messages.map((message, index) => (
                    <div
                      key={index}
                      className="flex items-start gap-3 bg-[#222222] p-4 rounded-lg"
                    >
                      {message.message_type === "Response" ? (
                        <Bot className="w-6 h-6 text-blue-400 mt-1" />
                      ) : (
                        <div className="w-6 h-6 rounded-full bg-gray-700 flex items-center justify-center">
                          <span className="text-xs text-white">U</span>
                        </div>
                      )}
                      <div className="flex-1">
                        <div className="text-sm text-gray-400 mb-1">
                          {new Date(message.timestamp).toLocaleString()}
                        </div>
                        <p className="text-white whitespace-pre-wrap">{message.content}</p>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
} 