import React, { useEffect, useState } from 'react';
import { getPeerConversations, Conversation } from './api/llm';
import { Bot } from 'lucide-react';

export function PeersConversation() {
  const [peerConversations, setPeerConversations] = useState<Record<string, Conversation>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchPeerConversations = async () => {
      try {
        const conversations = await getPeerConversations();
        setPeerConversations(conversations);
        setError(null);
      } catch (err) {
        setError('Failed to load peer conversations');
        console.error(err);
      } finally {
        setLoading(false);
      }
    };

    fetchPeerConversations();
    const interval = setInterval(fetchPeerConversations, 5000); // Refresh every 5 seconds
    return () => clearInterval(interval);
  }, []);

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="animate-spin rounded-full h-8 w-8 border-t-2 border-b-2 border-blue-500"></div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center min-h-screen text-red-500">
        {error}
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-[#111111] text-white">
      <div className="border-b border-gray-800 p-4">
        <div className="flex items-center justify-center gap-2">
          <Bot className="h-7 w-7 text-blue-400" />
          <h1 className="text-4xl font-bold text-white">Peer Conversations</h1>
        </div>
      </div>

      <div className="max-w-6xl mx-auto p-4 md:p-6">
        {Object.entries(peerConversations).length === 0 ? (
          <div className="text-center text-gray-400 mt-8">
            No peer conversations available
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-6">
            {Object.entries(peerConversations).map(([peerIp, conversation]) => (
              <div key={peerIp} className="bg-[#1a1a1a] rounded-lg p-6">
                <div className="mb-4 border-b border-gray-800 pb-4">
                  <h2 className="text-xl font-semibold text-blue-400">Peer: {peerIp}</h2>
                  <div className="text-sm text-gray-400 mt-2">
                    <div>Hostname: {conversation.host_info.hostname}</div>
                    <div>LLM Host: {conversation.host_info.is_llm_host ? 'Yes' : 'No'}</div>
                  </div>
                </div>

                <div className="space-y-4">
                  {conversation.messages.map((message, index) => (
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
            ))}
          </div>
        )}
      </div>
    </div>
  );
} 