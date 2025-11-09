import React from 'react';
import { Brain, MessageCircle, BookOpen, Code2, Sparkles } from 'lucide-react';
import { TypedWelcome } from './TypedWelcome';

export function WelcomeScreen({ onLoadSaved, savedCount, onCardClick }: { onLoadSaved: () => void; savedCount: number; onCardClick?: (question: string) => void; }) {
  const cards = [
    {
      icon: MessageCircle,
      title: 'Greeting',
      description: 'Start with a friendly hello',
      question: 'Hello! How are you doing today? Can you tell me a bit about yourself and what you can help me with?'
    },
    {
      icon: BookOpen,
      title: 'Learn',
      description: 'Ask about complex topics',
      question: 'I\'d like to learn something new today. Can you explain a complex topic in a simple and engaging way? Feel free to choose something interesting.'
    },
    {
      icon: Code2,
      title: 'Code',
      description: 'Get help with programming',
      question: 'I need help with programming. Can you help me write, debug, or explain code? I\'m working on a project and would appreciate your assistance.'
    },
    {
      icon: Sparkles,
      title: 'General',
      description: 'Ask anything you want',
      question: 'I have a question for you. Can you help me with anything you think might be useful or interesting?'
    }
  ];

  return (
    <div className="w-full h-full flex items-center justify-center">
      <div className="px-8 py-8 rounded-2xl bg-[#111111]/60 shadow-[0_0_20px_rgba(0,245,212,0.06)] border border-[#1a1a1a] backdrop-blur-md" style={{width: '640px', maxWidth: '90vw'}}>
        <Brain className="h-16 w-16 text-[#00f5d4] mx-auto mb-4 drop-shadow-[0_0_8px_#00f5d4] animate-float" />
        <TypedWelcome />
        <p className="text-gray-400 mb-4 text-lg text-center">
          Start a conversation with your AI assistant. Your message will be processed by the best available LLM in the network.
        </p>
        {/* Four Cards */}
        <div className="grid grid-cols-2 gap-3 mb-6 mt-6">
          {cards.map((card, index) => {
            const IconComponent = card.icon;
            return (
              <div
                key={index}
                onClick={() => onCardClick?.(card.question)}
                className="p-4 rounded-lg bg-[#0a0a0a]/80 border border-[#1a1a1a] hover:border-[#00f5d4]/30 transition-all cursor-pointer hover:shadow-[0_0_12px_rgba(0,245,212,0.1)] active:scale-95"
              >
                <IconComponent className="h-6 w-6 text-[#00f5d4] mb-2" />
                <h3 className="text-sm font-semibold text-white mb-1">{card.title}</h3>
                <p className="text-xs text-gray-400">{card.description}</p>
              </div>
            );
          })}
        </div>

        <div className="flex flex-col items-center">
          <button
            onClick={onLoadSaved}
            disabled={!(savedCount > 0)}
            className={`px-5 py-2 rounded-lg shadow-md transition-transform ${savedCount > 0 ? 'bg-gradient-to-r from-[#00f5d4] to-[#ff4ff8] text-black hover:scale-[1.02]' : 'bg-[#222] text-gray-400 cursor-not-allowed opacity-60'}`}
          >
            Load previous conversation
          </button>
          <div className="mt-2 text-xs text-gray-400">
            {savedCount > 0 ? (
              <span>{savedCount} messages available</span>
            ) : (
              <span>No saved conversation found</span>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}


