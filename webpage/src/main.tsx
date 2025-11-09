import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App.tsx';
import Login from './Login';
import { useEffect, useState } from 'react';
import { authStatus, logout } from './api/llm';
import './index.css';
import './styles/global.css';

function Root() {
  const [authed, setAuthed] = useState(false);
  const [username, setUsername] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const s = await authStatus();
        setAuthed(s.authenticated);
        setUsername(s.username ?? null);
      } catch {}
    })();
  }, []);

  if (!authed) return <Login onAuthenticated={(u) => { setAuthed(true); setUsername(u); }} />;

  const handleLogout = async () => {
    try { await logout(); } catch {}
    setAuthed(false);
    setUsername(null);
  };

  return <App username={username ?? undefined} onLogout={handleLogout} />;
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Root />
  </StrictMode>
);
