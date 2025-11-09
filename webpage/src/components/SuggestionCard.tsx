import { ReactNode } from 'react';

interface SuggestionCardProps {
  icon: ReactNode;
  title: string;
  description: string;
  onClick: () => void;
}

export const SuggestionCard = ({ icon, title, description, onClick }: SuggestionCardProps) => {
  return (
    <button
      onClick={onClick}
      className="group relative overflow-hidden p-4 bg-[#111111] rounded-xl text-left border border-[#1f1f1f] 
                shadow-md transition-all duration-200 hover:scale-105 hover:border-[#00f5d4] hover:shadow-[0_0_10px_#00f5d4]
                flex items-center gap-3"
      style={{ minWidth: '260px' }}
    >
      {/* Glow effect on hover */}
      <div className="absolute inset-0 bg-gradient-to-r from-neon/0 via-neon/5 to-neon/0 
                    opacity-0 group-hover:opacity-100 transition-opacity duration-500
                    translate-x-[-100%] group-hover:translate-x-[100%]" />
      
      {/* Icon with pulse effect */}
      <div className="text-2xl">{icon}</div>
      <div className="text-left">
        <div className="text-[#00f5d4] font-semibold">{title}</div>
        <div className="text-gray-400 text-sm">{description}</div>
      </div>
      
      {/* Border glow effect */}
      <div className="absolute inset-0 border border-transparent group-hover:border-[#00f5d4]/30 rounded-xl transition-colors duration-200" />
    </button>
  );
};