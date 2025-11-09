import { useState, useRef, useEffect, Suspense, lazy } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { fadeInUp, slideInLeft, staggerContainer, pageFade } from './animations';
import { Send, Bot, Loader2, Paperclip, Brain, MessageSquare } from 'lucide-react';
import { sendMessageToLLM, getLocalConversation } from './api/llm';
// Lazy-loaded pages for better code-splitting
const PeersConversationLazy = lazy(() => import('./PeersConversation').then(m => ({ default: m.PeersConversation })));
import { FileUpload } from './components/FileUpload';
import { NetworkBackground } from './components/NetworkBackground';
import { Navbar } from './components/Navbar';
const AnalyticsDashboardLazy = lazy(() => import('./components/AnalyticsDashboard'));
import { WelcomeScreen } from './components/WelcomeScreen';
import { FileSidebar } from './components/FileSidebar';

type Message = { 
  role: 'user' | 'assistant'; 
  content: string; 
  timestamp?: string;
  hostInfo?: {
    hostname: string;
    ip_address: string;
    is_llm_host: boolean;
  };
  fileInfo?: {
    filename: string;
    file_type: string;
    file_size: number;
    uploader_ip: string;
    upload_time: string;
  };
};

function useNetworkStatus() {
  const [peerCount, setPeerCount] = useState(0);
  const [isLLMHost, setIsLLMHost] = useState(false);
  useEffect(() => {
    const checkStatus = async () => {
      try {
        const response = await fetch('/api/status');
        const data = await response.json();
        if (typeof data.peer_count === 'number') setPeerCount(data.peer_count);
        if (typeof data.is_llm_host === 'boolean') setIsLLMHost(data.is_llm_host);
      } catch (error) {
        setPeerCount(0);
        setIsLLMHost(false);
      }
    };
    checkStatus();
    const interval = setInterval(checkStatus, 5000);
    return () => clearInterval(interval);
  }, []);
  return { peerCount, isLLMHost };
}

function ChatPage({ 
  onNavigate,
  conversation,
  setConversation,
  isTyping,
  setIsTyping,
  savedLocal,
  loadSavedLocal,
  username,
  onLogout,
}: {
  onNavigate: (page: string) => void;
  conversation: Message[];
  setConversation: (messages: Message[]) => void;
  isTyping: boolean;
  setIsTyping: (typing: boolean) => void;
  savedLocal: any | null;
  loadSavedLocal: () => void;
  username?: string;
  onLogout?: () => void;
}) {
  const { peerCount, isLLMHost } = useNetworkStatus();
  const [inputValue, setInputValue] = useState('');
  const [showFileUpload, setShowFileUpload] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  };

  useEffect(() => {
    scrollToBottom();
  }, [conversation]);

  // No global body overflow locking — allow landing to scroll.
  useEffect(() => {}, [conversation.length]);

  const handleSendMessage = async () => {
    if (inputValue.trim() === '' || isTyping) return;
    
    const newMessage = { role: 'user' as const, content: inputValue };
    const newConversation = [...conversation, newMessage];
    
    setConversation(newConversation);
    setInputValue('');
    setIsTyping(true);
    
    try {
      const response = await sendMessageToLLM(inputValue);
      setIsTyping(false);
      setConversation([
        ...newConversation,
        { role: 'assistant' as const, content: response }
      ]);
    } catch (error) {
      console.error('Error getting LLM response:', error);
      setIsTyping(false);
      setConversation([
        ...newConversation,
        { role: 'assistant' as const, content: 'Sorry, I encountered an error while processing your request.' }
      ]);
    }
  };

  // Send current input with an attached filename
  const handleSendMessageWithFilename = async (filename: string) => {
    if (inputValue.trim() === '' || isTyping) return;
    const newMessage = { role: 'user' as const, content: inputValue };
    const newConversation = [...conversation, newMessage];
    setConversation(newConversation);
    setInputValue('');
    setIsTyping(true);
    try {
      const response = await sendMessageToLLM(newMessage.content, filename);
      setIsTyping(false);
      setConversation([...newConversation, { role: 'assistant' as const, content: response }]);
    } catch (error) {
      console.error('Error getting LLM response:', error);
      setIsTyping(false);
      setConversation([...newConversation, { role: 'assistant' as const, content: 'Sorry, I encountered an error while processing your request.' }]);
    }
  };

  // Listen for ask-file events dispatched from SharedFilesPanel
  useEffect(() => {
    const onAsk = (e: Event) => {
      const ce = e as CustomEvent<{ filename: string }>;
      const f = ce.detail?.filename;
      if (!f) return;
      setInputValue(`Explain the contents of ${f}`);
      setTimeout(() => handleSendMessageWithFilename(f), 50);
    };
    window.addEventListener('meshmind:ask-file', onAsk as EventListener);
    return () => window.removeEventListener('meshmind:ask-file', onAsk as EventListener);
  }, [conversation, isTyping, inputValue]);

  const handleFileUploaded = (fileInfo: any) => {
    const fileMessage: Message = {
      role: 'user',
      content: `📎 Shared file: ${fileInfo.filename}`,
      timestamp: new Date().toISOString(),
      fileInfo: fileInfo
    };
    setConversation([...conversation, fileMessage]);
    setShowFileUpload(false);
  };

  // removed file-related ask handlers (unused)

  return (
    <div className="h-screen w-full text-white font-inter flex flex-col overflow-hidden" style={{background: '#0b0b0b'}}>
      {/* Fixed Navbar - always on top */}
      <div className="flex-shrink-0 relative z-50 h-16">
        <Navbar peerCount={peerCount} isLLMHost={isLLMHost} onNavigatePeers={() => onNavigate('peers')} onNavigateAnalytics={() => onNavigate('analytics')} username={username} onLogout={onLogout} />
      </div>

      {/* Main content area - flex layout with sidebar and chat */}
      <motion.div className="flex-1 flex gap-4 overflow-hidden relative" initial="initial" animate="animate" variants={staggerContainer(0.08)}>
        {/* Left: Sidebar - fixed width, fully visible, starts below navbar */}
        <motion.div variants={slideInLeft} className="w-[320px] flex-shrink-0 flex flex-col border-r border-divider bg-surface/40 transition-all duration-300 will-change-transform will-change-opacity z-20">
          <div className="flex-1 overflow-auto p-4 h-[calc(100vh-4rem)]" style={{scrollbarWidth: 'auto', scrollbarColor: '#00bfa6 #1a1a1a'}}>
            <FileSidebar />
          </div>
        </motion.div>

        {/* Center: Chat area - flex-grow to fill remaining space */}
        <motion.div variants={fadeInUp} className="flex-1 flex flex-col items-center justify-start px-4 py-6 overflow-hidden will-change-transform will-change-opacity">
          {/* Chat container - proper height with scroll containment */}
          <div className="w-full h-full flex flex-col rounded-2xl border border-divider bg-panel/80 backdrop-blur-sm shadow-soft overflow-hidden z-0" style={{maxWidth: 'calc(100% - 2rem)'}}>
            {/* Welcome or Messages */}
            <div className="flex-1 flex flex-col overflow-hidden">
              {conversation.length === 0 ? (
                <div className="flex-1 relative overflow-y-auto">
                  <NetworkBackground />
                  <div className="absolute inset-0 flex flex-col items-center justify-center">
                    <WelcomeScreen 
                      onLoadSaved={loadSavedLocal} 
                      savedCount={(savedLocal && savedLocal.messages) ? savedLocal.messages.length : 0}
                      onCardClick={(question) => {
                        setInputValue(question);
                      }}
                    />
                  </div>
                </div>
              ) : (
                <div className="flex-1 overflow-y-auto px-4 py-4 space-y-3 scroll-smooth min-h-0">
                  <AnimatePresence initial={false}>
                    {conversation.map((message, index) => (
                      <motion.div
                        key={index}
                        initial={{ opacity: 0, y: 10 }}
                        animate={{ opacity: 1, y: 0, transition: { duration: 0.25, ease: 'easeInOut' } }}
                        exit={{ opacity: 0, y: 6, transition: { duration: 0.2, ease: 'easeInOut' } }}
                        className={`flex items-start gap-3 py-3 px-4 rounded-lg transition-all duration-200 bg-surface border border-divider`}
                      >
                        {message.role === 'assistant' ? (
                          <Bot className="w-6 h-6 text-accent mt-1 flex-shrink-0" />
                        ) : (
                          <div className="w-6 h-6 rounded-full bg-surface-2 border border-divider flex items-center justify-center flex-shrink-0">
                            <span className="text-xs text-white font-medium">You</span>
                          </div>
                        )}
                        <div className="flex-1 min-w-0">
                          <p className="text-bright whitespace-pre-wrap leading-relaxed text-sm">{message.content}</p>
                          {message.fileInfo?.filename && (
                            <div className="mt-2">
                              <button
                                onClick={() => {
                                  const fname = message.fileInfo!.filename;
                                  setInputValue(`Explain the contents of ${fname}`);
                                  setTimeout(() => handleSendMessageWithFilename(fname), 50);
                                }}
                                className="inline-flex items-center gap-1 px-2 py-1 rounded-md text-xs bg-panel hover:bg-panel-hover border border-divider text-bright/90"
                              >
                                <MessageSquare className="h-3.5 w-3.5 text-accent" />
                                Ask about this file
                              </button>
                            </div>
                          )}
                        </div>
                      </motion.div>
                    ))}
                  </AnimatePresence>
                  {isTyping && (
                    <div className="flex items-center gap-3 py-3 px-4 rounded-lg bg-surface border border-divider">
                      <Bot className="w-6 h-6 text-accent flex-shrink-0" />
                      <div className="flex items-center gap-2">
                        <Loader2 className="w-4 h-4 text-gray-400 animate-spin" />
                        <span className="text-gray-400 text-sm">Thinking...</span>
                      </div>
                    </div>
                  )}
                  <div ref={messagesEndRef} />
                </div>
              )}
            </div>

            {/* Input area - fixed at bottom */}
            <div className="flex-shrink-0 border-t border-divider bg-panel/80 p-4 space-y-2 z-10 backdrop-blur-sm">
              {showFileUpload && (
                <div className="mb-3">
                  <FileUpload onFileUploaded={handleFileUploaded} />
                </div>
              )}
              <form 
                onSubmit={(e) => { 
                  e.preventDefault(); 
                  handleSendMessage(); 
                }} 
                className="space-y-2"
              >
                <div className="relative">
                  <textarea
                    value={inputValue}
                    onChange={(e) => setInputValue(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' && !e.shiftKey) {
                        e.preventDefault();
                        handleSendMessage();
                      }
                    }}
                    placeholder="Ask anything... (Shift+Enter for new line)"
                    className="w-full bg-surface border border-divider text-bright px-4 py-3 pr-20 rounded-lg focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/20 resize-none min-h-[40px] max-h-20 shadow transition-all duration-200 overflow-hidden"
                    style={{fontSize: '0.95rem', height: 'auto'}}
                    rows={1}
                    onInput={(e) => {
                      const target = e.target as HTMLTextAreaElement;
                      target.style.height = 'auto';
                      target.style.height = Math.min(target.scrollHeight, 128) + 'px';
                    }}
                  />
                  <motion.button
                    type="button"
                    onClick={() => setShowFileUpload(!showFileUpload)}
                    className="absolute right-12 bottom-3 h-8 w-8 flex items-center justify-center text-gray-400 hover:text-accent transition-colors duration-200"
                    title="Attach file"
                    whileHover={{ scale: 1.05 }} whileTap={{ scale: 0.98 }}
                  >
                    <Paperclip className="h-5 w-5" />
                  </motion.button>
                  <motion.button
                    type="submit"
                    disabled={isTyping || !inputValue.trim()}
                    className="absolute right-3 bottom-3 h-8 w-8 flex items-center justify-center bg-accent hover:bg-accent/90 disabled:opacity-50 disabled:hover:bg-accent rounded-full transition-all duration-200 shadow-lg"
                    whileTap={{ scale: 0.95 }}
                  >
                    <Send className="h-5 w-5 text-black" />
                  </motion.button>
                </div>
                <div className="flex justify-between items-center text-xs text-gray-500">
                  <span>Press Enter to send, Shift+Enter for new line</span>
                  <span className={inputValue.length > 1000 ? 'text-accent' : ''}>
                    {inputValue.length}/2000
                  </span>
                </div>
              </form>
            </div>
          </div>
        </motion.div>

      </motion.div>
    </div>
  );
}

function App({ username, onLogout }: { username?: string; onLogout?: () => void }) {
  const [currentPage, setCurrentPage] = useState('chat');
  const [conversation, setConversation] = useState<Message[]>([]);
  const [isTyping, setIsTyping] = useState(false);

  // Fetch saved local conversation into a separate state (do not auto-load into chat)
  const [savedLocal, setSavedLocal] = useState<any | null>(null);

  useEffect(() => {
    const fetchLocal = async () => {
      try {
        const local = await getLocalConversation();
        setSavedLocal(local);
      } catch (err) {
        console.error('Failed to fetch local conversation:', err);
      }
    };
    fetchLocal();
  }, []);

  const loadSavedLocal = () => {
    if (!savedLocal || !savedLocal.messages) return;
    const mapped = savedLocal.messages.map((m: any) => ({
      role: m.message_type === 'Response' ? 'assistant' as const : 'user' as const,
      content: m.content,
      timestamp: m.timestamp,
      hostInfo: m.host_info,
    }));
    setConversation(mapped);
  };

  return (
    <AnimatePresence mode="wait">
    <motion.div key={currentPage} variants={pageFade} initial="initial" animate="animate" exit="exit" className="bg-dark">
      {currentPage === 'chat' ? (
        <ChatPage 
          onNavigate={setCurrentPage} 
          conversation={conversation}
          setConversation={setConversation}
          isTyping={isTyping}
          setIsTyping={setIsTyping}
          savedLocal={savedLocal}
          loadSavedLocal={loadSavedLocal}
          username={username}
          onLogout={onLogout}
        />
      ) : currentPage === 'analytics' ? (
        <div className="min-h-screen text-white">
          <div className="fixed top-4 left-4 z-10">
            <button
              onClick={() => setCurrentPage('chat')}
              className="flex items-center gap-2 px-4 py-2 bg-panel hover:bg-panel-hover rounded-lg text-white border border-[#232323]"
            >
              <Brain className="h-5 w-5" />
              <span>Back to Chat</span>
            </button>
          </div>
          <div className="pt-16">
            <Suspense fallback={<div className="p-6 text-gray-400">Loading analytics…</div>}>
              <AnalyticsDashboardLazy />
            </Suspense>
          </div>
        </div>
      ) : (
        <div>
          <div className="fixed top-4 left-4 z-10">
            <button
              onClick={() => setCurrentPage('chat')}
              className="flex items-center gap-2 px-4 py-2 bg-panel hover:bg-panel-hover rounded-lg text-white"
            >
              <Brain className="h-5 w-5" />
              <span>Back to Chat</span>
            </button>
          </div>
          <Suspense fallback={<div className="p-6 text-gray-400">Loading peers…</div>}>
            <PeersConversationLazy />
          </Suspense>
        </div>
      )}
    </motion.div>
    </AnimatePresence>
  );
}

export default App;