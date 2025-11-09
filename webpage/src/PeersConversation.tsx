import { useEffect, useState } from "react";
import { getPeerConversations, Conversation } from "./api/llm";
import { Bot } from "lucide-react";

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
        console.log("Received peer conversations:", conversations);
        setPeerConversations(conversations);
        setError(null);

        const peerIps = Object.keys(conversations);
        // Preserve the currently selected peer if it still exists; otherwise pick the first.
        setSelectedPeer((prev) => {
          if (prev && peerIps.includes(prev)) return prev;
          return peerIps.length > 0 ? peerIps[0] : null;
        });
      } catch (err) {
        console.error("Error loading peer conversations:", err);
        setError("⚠️ Failed to load peer conversations. Please try again.");
      } finally {
        setLoading(false);
      }
    };

    fetchPeerConversations();
    const interval = setInterval(fetchPeerConversations, 5000);
    return () => clearInterval(interval);
  }, []);

  const peerIps = Object.keys(peerConversations);
  const getPeerLabel = (ip: string) => peerConversations[ip]?.host_info?.hostname || ip;

  return (
    <div className="min-h-screen bg-dark text-bright font-inter">
      {/* Header */}
      <div className="border-b border-divider p-5 backdrop-blur-sm bg-surface-2/70">
        <div className="flex items-center justify-center gap-3">
          <Bot className="h-8 w-8 text-accent" />
          <h1 className="text-3xl font-semibold text-bright">Peer Conversations</h1>
        </div>
      </div>

      <div className="max-w-6xl mx-auto p-6">
        {/* Loading & Errors */}
        {loading && peerIps.length === 0 ? (
          <div className="flex justify-center mt-20">
            <div className="animate-spin rounded-full h-10 w-10 border-t-2 border-b-2 border-accent"></div>
          </div>
        ) : error ? (
          <div className="text-center text-red-400 bg-red-900/20 rounded-lg p-4 border border-red-700/40">
            {error}
          </div>
        ) : peerIps.length === 0 ? (
          <div className="text-center text-dim mt-16">
            No peer conversations available
          </div>
        ) : (
          <div className="grid grid-cols-1 gap-6">
            {/* Peer Selector */}
            <div className="flex flex-wrap gap-4 mb-8 justify-center">
              {peerIps.map((ip) => (
                <button
                  key={ip}
                  onClick={() => setSelectedPeer(ip)}
                  className={`px-5 py-2.5 rounded-md text-sm font-medium transition-colors border 
                    ${selectedPeer === ip
                      ? 'bg-accent text-black border-accent'
                      : 'bg-panel text-bright border-divider hover:border-accent/60'}
                  `}
                >
                  {getPeerLabel(ip)}
                </button>
              ))}
            </div>

            {/* Conversation Panel */}
            {selectedPeer && peerConversations[selectedPeer] && (
              <div className="bg-panel/80 rounded-2xl p-6 border border-divider backdrop-blur-sm shadow-soft">
                <div className="mb-5 border-b border-divider pb-4">
                  <h2 className="text-2xl font-semibold text-accent">
                    Peer: {getPeerLabel(selectedPeer)}
                  </h2>
                  <div className="text-sm text-dim mt-2">
                    <div>Hostname: {peerConversations[selectedPeer].host_info.hostname} ({selectedPeer})</div>
                    <div>
                      LLM Host:{" "}
                      <span
                        className={`font-semibold ${peerConversations[selectedPeer].host_info.is_llm_host ? 'text-accent' : 'text-dim'}`}
                      >
                        {peerConversations[selectedPeer].host_info.is_llm_host ? "Yes" : "No"}
                      </span>
                    </div>
                  </div>
                </div>

                <div className="space-y-5">
                  {peerConversations[selectedPeer].messages.map((message, index) => (
                    <div
                      key={index}
                      className={`flex items-start gap-4 p-4 rounded-xl transition-colors bg-surface border border-divider`}
                    >
                      {message.message_type === "Response" ? (
                        <Bot className="w-6 h-6 text-accent" />
                      ) : (
                        <div className="w-6 h-6 rounded-full bg-surface-2 border border-divider flex items-center justify-center text-white text-xs font-medium">U</div>
                      )}
                      <div className="flex-1">
                        <div className="text-xs text-dim mb-1">
                          {new Date(message.timestamp).toLocaleString()}
                        </div>
                        <p className="text-bright whitespace-pre-wrap leading-relaxed">
                          {message.content}
                        </p>
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
