import { useState, useEffect } from 'react';
import { motion } from 'framer-motion';
import { fadeInUp } from './animations';
import { authStatus, login } from './api/llm';
import { Brain } from 'lucide-react';
import { NetworkBackground } from './components/NetworkBackground';

export default function Login({ onAuthenticated }: { onAuthenticated: (username: string) => void }) {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    (async () => {
      try {
        const s = await authStatus();
        if (s.authenticated && s.username) onAuthenticated(s.username);
      } catch {}
      setLoading(false);
    })();
  }, [onAuthenticated]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    try {
      const res = await login(username, password);
      if (res.authenticated && res.username) {
        onAuthenticated(res.username);
      } else {
        setError('Invalid credentials');
      }
    } catch (e: any) {
      setError('Login failed');
    }
  };

  if (loading) return <div className="min-h-screen flex items-center justify-center text-gray-300">Loading…</div>;

  return (
    <div className="min-h-screen relative flex items-center justify-center bg-dark overflow-hidden">
      <div className="absolute inset-0">
        <NetworkBackground />
      </div>
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(600px_circle_at_center,rgba(0,191,166,0.08),transparent_60%)]" />
      <div className="pointer-events-none absolute inset-0 bg-black/10" />
      <motion.form
        initial="initial"
        animate="animate"
        variants={fadeInUp}
        onSubmit={handleSubmit}
        className="relative z-10 w-full max-w-sm p-6 rounded-xl bg-white/5 backdrop-blur-md border border-white/10 shadow-soft will-change-transform will-change-opacity"
      >
        <div className="mb-4 flex items-center justify-center gap-2">
          <Brain className="h-7 w-7 text-accent" />
          <span className="text-2xl font-semibold text-bright">MeshMind</span>
        </div>
        <p className="text-center text-dim text-sm mb-5">Enterprise AI Network</p>
        <h1 className="text-base font-medium text-bright/90 mb-3">Sign in</h1>
        <div className="space-y-3">
          <div>
            <label className="block text-sm text-dim mb-1">Username</label>
            <input value={username} onChange={e=>setUsername(e.target.value)} className="w-full bg-surface border border-divider text-bright px-3 py-2 rounded-md focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/20" />
          </div>
          <div>
            <label className="block text-sm text-dim mb-1">Password</label>
            <input type="password" value={password} onChange={e=>setPassword(e.target.value)} className="w-full bg-surface border border-divider text-bright px-3 py-2 rounded-md focus:outline-none focus:border-accent focus:ring-1 focus:ring-accent/20" />
          </div>
        </div>
        {error && <div className="mt-3 text-sm text-red-400">{error}</div>}
        <motion.button
          whileHover={{ scale: 1.05 }}
          whileTap={{ scale: 0.98 }}
          type="submit"
          className="mt-5 w-full py-2 bg-gradient-to-r from-accent to-accent-secondary text-black rounded-md shadow-soft hover:shadow-glow transition-shadow"
        >
          Login
        </motion.button>
      </motion.form>
    </div>
  );
}
