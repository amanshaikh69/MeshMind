import { useState, useEffect } from 'react';

export const TypedWelcome = () => {
  const [text, setText] = useState('');
  const fullText = 'Welcome to MeshMind🧠';
  
  useEffect(() => {
    let currentIndex = 0;
    const interval = setInterval(() => {
      if (currentIndex <= fullText.length) {
        setText(fullText.slice(0, currentIndex));
        currentIndex++;
      } else {
        clearInterval(interval);
      }
    }, 100);

    return () => clearInterval(interval);
  }, []);

  return (
    <h2 className="text-3xl font-bold text-bright mb-2 tracking-wide text-center">
      {text}
      <span className="animate-blink">|</span>
    </h2>
  );
};