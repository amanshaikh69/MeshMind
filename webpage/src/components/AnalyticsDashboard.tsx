import React, { useEffect, useMemo, useState } from 'react';
import { BarChart3, Users, Clock3, Network, Download } from 'lucide-react';
import { LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer, PieChart, Pie, Cell, Legend } from 'recharts';

function KPI({ label, value, icon, hint }: { label: string; value: string; icon: React.ReactNode; hint?: string }) {
  return (
    <div className="flex items-center justify-between p-5 rounded-xl border border-divider bg-panel/80 min-h-[118px]">
      <div className="flex flex-col">
        <div className="text-[11px] text-dim">{label}</div>
        <div className="text-2xl font-semibold text-bright mt-1">{value}</div>
        {hint && <div className="text-[11px] text-dim mt-1">{hint}</div>}
      </div>
      <div className="p-2 rounded-lg bg-surface border border-divider text-accent">{icon}</div>
    </div>
  );
}

function DeltaBadge({ value }: { value: number }) {
  const up = value >= 0;
  const txt = `${up ? '+' : ''}${value.toFixed(1)}%`;
  return (
    <span className={`ml-2 text-[10px] px-1.5 py-0.5 rounded ${up ? 'bg-emerald-900/40 text-emerald-300' : 'bg-rose-900/40 text-rose-300'}`}>{txt}</span>
  );
}

const formatBytes = (n: number) => {
  if (n < 1024) return `${n} B`;
  const units = ['KB','MB','GB','TB'];
  let i = -1; let v = n;
  do { v /= 1024; i++; } while (v >= 1024 && i < units.length - 1);
  return `${v.toFixed(v >= 100 ? 0 : v >= 10 ? 1 : 2)} ${units[i]}`;
};

const formatNum = (n: number) => n.toLocaleString();
const FILE_COLORS = ['#00bfa6', '#5b8aff', '#a88bfa', '#7dd3fc', '#34d399'];

export default function AnalyticsDashboard() {
  const [now] = useState(() => new Date());
  // Chat stats from backend
  const [messagesPerDay, setMessagesPerDay] = useState<{date: string; count: number}[]>([]);
  const [topUsers, setTopUsers] = useState<{user: string; count: number}[]>([]);
  // File stats from backend
  const [fileTypes, setFileTypes] = useState<{type: string; count: number; total_bytes: number}[]>([]);
  const [largestFiles, setLargestFiles] = useState<{filename: string; bytes: number; uploader_ip: string; file_type: string}[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [range, setRange] = useState<'7d' | '30d'>('7d');

  // Engagement / Network / Perf state
  const [engagement, setEngagement] = useState<{dau:number; wau:number; avg_session_seconds:number} | null>(null);
  const [network, setNetwork] = useState<{latency_ms?: {p50:number|null;p95:number|null;p99:number|null}} | null>(null);

  // Section refs for scrolling
  const messagesRef = React.useRef<HTMLDivElement>(null);
  const filesRef = React.useRef<HTMLDivElement>(null);
  const networkRef = React.useRef<HTMLDivElement>(null);
  const engagementRef = React.useRef<HTMLDivElement>(null);

  const scrollTo = (ref: React.RefObject<HTMLElement>) => ref.current?.scrollIntoView({ behavior: 'smooth', block: 'start' });

  const loadData = async () => {
    try {
      setError(null);
      const [chatRes, filesRes, engagementRes, networkRes] = await Promise.all([
        fetch('/api/analytics/chat'),
        fetch('/api/analytics/files'),
        fetch('/api/analytics/engagement'),
        fetch('/api/analytics/network'),
      ]);
      const chat = await chatRes.json();
      const files = await filesRes.json();
      const engagementJson = await engagementRes.json();
      const networkJson = await networkRes.json();
      setMessagesPerDay(Array.isArray(chat?.messages_per_day) ? chat.messages_per_day : []);
      setTopUsers(Array.isArray(chat?.top_users) ? chat.top_users : []);
      setFileTypes(Array.isArray(files?.types) ? files.types : []);
      setLargestFiles(Array.isArray(files?.largest) ? files.largest : []);
      setEngagement(engagementJson || null);
      setNetwork(networkJson || null);
    } catch (e: any) {
      setError(e?.message || 'Failed to load analytics');
    } finally {
      setLoading(false);
    }
  };

  // Initial + 30s polling
  useEffect(() => {
    loadData();
    const id = setInterval(loadData, 30000);
    return () => clearInterval(id);
  }, []);

  // Derive sparkline series from messagesPerDay (fallback to mock)
  const series = useMemo(() => {
    const cutoff = new Date();
    cutoff.setDate(cutoff.getDate() - (range === '7d' ? 7 : 30));
    const inRange = messagesPerDay.filter(d => {
      const dt = new Date(d.date);
      return dt >= cutoff;
    });
    const arr = inRange.map((d) => d.count);
    if (arr.length >= 2) return arr;
    return [3, 5, 2, 8, 6, 9, 7];
  }, [messagesPerDay, range]);

  const messagesTotal = useMemo(() => series.reduce((s, v) => s + v, 0), [series]);
  const messagesDeltaPct = useMemo(() => {
    if (series.length < 2) return 0;
    const half = Math.floor(series.length / 2);
    const prev = series.slice(0, half).reduce((s, v) => s + v, 0);
    const curr = series.slice(half).reduce((s, v) => s + v, 0);
    return prev === 0 ? 100 : ((curr - prev) / prev) * 100;
  }, [series]);

  // Data for charts and KPIs
  const messagesData = useMemo(() => {
    const cutoff = new Date();
    cutoff.setDate(cutoff.getDate() - (range === '7d' ? 7 : 30));
    return messagesPerDay
      .filter((d) => new Date(d.date) >= cutoff)
      .map((d) => ({
        date: new Date(d.date).toLocaleDateString(undefined, { month: 'short', day: '2-digit' }),
        count: d.count,
      }));
  }, [messagesPerDay, range]);

  const filesTotalCount = useMemo(() => fileTypes.reduce((s, ft) => s + (ft.count || 0), 0), [fileTypes]);
  const filesTotalBytes = useMemo(() => fileTypes.reduce((s, ft) => s + (ft.total_bytes || 0), 0), [fileTypes]);

  return (
    <div className="w-full px-6 py-6">
      <div className="max-w-7xl mx-auto space-y-6">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-2xl font-semibold text-accent">Analytics Dashboard</h2>
            <div className="text-xs text-dim">Auto-refreshing every 30 seconds · {now.toLocaleDateString()}</div>
          </div>
          <div className="flex gap-2">
            <button onClick={() => window.print()} className="px-3 py-2 text-sm rounded-md bg-accent text-black hover:bg-accent/90 transition-colors flex items-center gap-2">
              <Download className="w-4 h-4" /> Export PDF
            </button>
          </div>
        </div>

        {/* Controls */}
        <div className="flex items-center justify-between">
          <div className="text-xs text-dim">Select range to compare trends</div>
          <div className="inline-flex rounded-md border border-divider bg-panel/80 overflow-hidden">
            <button onClick={() => setRange('7d')} className={`px-3 py-1.5 text-xs ${range==='7d'?'bg-surface text-accent':'text-bright/80 hover:bg-surface'}`}>Last 7 days</button>
            <button onClick={() => setRange('30d')} className={`px-3 py-1.5 text-xs border-l border-divider ${range==='30d'?'bg-surface text-accent':'text-bright/80 hover:bg-surface'}`}>Last 30 days</button>
          </div>
        </div>

        {/* Loading / Error */}
        {loading && (
          <div className="text-xs text-dim">Loading analytics…</div>
        )}
        {error && (
          <div className="text-xs text-rose-400">{error}</div>
        )}

        {/* KPI Row */}
        <div className="grid grid-cols-2 md:grid-cols-3 xl:grid-cols-5 gap-4">
          <div title="Total messages in selected range across all peers" onClick={() => scrollTo(messagesRef)} className="cursor-pointer">
            <KPI label={`Messages (${range})`} value={formatNum(messagesTotal)} icon={<BarChart3 className="w-5 h-5" />} hint="Across all peers" />
            <DeltaBadge value={messagesDeltaPct} />
          </div>
          <div title="Distinct IPs observed in data window" onClick={() => scrollTo(engagementRef)} className="cursor-pointer">
            <KPI label="Active Users" value={`${topUsers.length}`} icon={<Users className="w-5 h-5" />} hint="Distinct IPs observed" />
          </div>
          <div title="Total uploaded files and storage footprint" onClick={() => scrollTo(filesRef)} className="cursor-pointer">
            <KPI label="Files" value={formatNum(filesTotalCount)} icon={<BarChart3 className="w-5 h-5" />} hint={`Total size: ${formatBytes(filesTotalBytes)}`} />
          </div>
          <div title="P95 latency over last hour" onClick={() => scrollTo(networkRef)} className="cursor-pointer">
            <KPI label="Avg. Latency" value={network?.latency_ms?.p95 != null ? `${network?.latency_ms?.p95} ms` : '—'} icon={<Network className="w-5 h-5" />} hint="From /api/analytics/network" />
          </div>
          <div title="Average session duration (10m idle)" onClick={() => scrollTo(engagementRef)} className="cursor-pointer">
            <KPI label="Avg. Session" value={engagement ? `${Math.floor(engagement.avg_session_seconds/60)}m ${engagement.avg_session_seconds%60}s` : '—'} icon={<Clock3 className="w-5 h-5" />} hint="From /api/analytics/engagement" />
          </div>
        </div>

        {/* Charts Row (dependency-free placeholders) */}
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
          <div ref={messagesRef} className="rounded-2xl border border-divider bg-panel/80 p-4">
            <div className="flex items-center justify-between">
              <div className="text-sm font-semibold text-bright">Messages per day</div>
              <span className="text-xs text-dim">{range === '7d' ? 'last 7 days' : 'last 30 days'}</span>
            </div>
            <div className="mt-3 h-40">
              <ResponsiveContainer width="100%" height="100%">
                <LineChart data={messagesData} margin={{ top: 10, right: 10, left: 0, bottom: 0 }}>
                  <XAxis dataKey="date" tick={{ fill: '#9ba0a8', fontSize: 11 }} axisLine={{ stroke: '#2a2d33' }} tickLine={{ stroke: '#2a2d33' }} />
                  <YAxis tick={{ fill: '#9ba0a8', fontSize: 11 }} axisLine={{ stroke: '#2a2d33' }} tickLine={{ stroke: '#2a2d33' }} allowDecimals={false} />
                  <Tooltip contentStyle={{ background: '#1e2026', border: '1px solid #2a2d33', color: '#e8eaed' }} labelStyle={{ color: '#e8eaed' }} itemStyle={{ color: '#e8eaed' }} />
                  <Legend wrapperStyle={{ color: '#e8eaed' }} />
                  <Line type="monotone" dataKey="count" name="Messages" stroke="#00bfa6" strokeWidth={2} dot={false} />
                </LineChart>
              </ResponsiveContainer>
            </div>
          </div>

          <div ref={filesRef} className="rounded-2xl border border-divider bg-panel/80 p-4">
            <div className="flex items-center justify-between">
              <div className="text-sm font-semibold text-bright">File usage by type</div>
              <span className="text-xs text-dim">live</span>
            </div>
            <div className="mt-2 h-48">
              <ResponsiveContainer width="100%" height="100%">
                <PieChart>
                  <Pie
                    data={(fileTypes.length ? fileTypes : []).map(ft => ({ name: ft.type.toUpperCase(), value: ft.total_bytes }))}
                    dataKey="value"
                    nameKey="name"
                    innerRadius={40}
                    outerRadius={70}
                    paddingAngle={2}
                  >
                    {(fileTypes.length ? fileTypes : []).map((_, idx) => (
                      <Cell key={`cell-${idx}`} fill={FILE_COLORS[idx % FILE_COLORS.length]} />
                    ))}
                  </Pie>
                  <Tooltip formatter={(v: any) => formatBytes(Number(v))} contentStyle={{ background: '#1e2026', border: '1px solid #2a2d33', color: '#e8eaed' }} labelStyle={{ color: '#e8eaed' }} itemStyle={{ color: '#e8eaed' }} />
                  <Legend wrapperStyle={{ color: '#e8eaed' }} />
                </PieChart>
              </ResponsiveContainer>
            </div>
            {fileTypes.length > 0 && (
              <div className="mt-3 text-[11px] text-dim">
                Total size: {formatBytes(fileTypes.reduce((s, ft) => s + ft.total_bytes, 0))}
              </div>
            )}
          </div>

          <div ref={networkRef} className="rounded-2xl border border-divider bg-panel/80 p-4">
            <div className="flex items-center justify-between">
              <div className="text-sm font-semibold text-bright">Network stats</div>
              <span className="text-xs text-dim">live</span>
            </div>
            <div className="mt-4 grid grid-cols-1 gap-3">
              <KPI label="Latency (P95)" value={network?.latency_ms?.p95 != null ? `${network?.latency_ms?.p95} ms` : '—'} icon={<Network className="w-4 h-4" />} />
            </div>
          </div>
        </div>
        {/* Tables Row */}
        <div className="grid grid-cols-1 xl:grid-cols-1 gap-4">
          <div ref={engagementRef} className="rounded-2xl border border-divider bg-panel/80 p-4">
            <div className="text-sm font-semibold text-bright mb-1">Top active users</div>
            <div className="text-xs text-dim mb-3">Distinct IPs with highest message counts</div>
            <div className="flex justify-end mb-2">
              <button
                className="text-[11px] px-2 py-1 border border-divider rounded bg-panel hover:bg-panel/80"
                onClick={() => {
                  const rows = (topUsers.length ? topUsers : []).map(u => [u.user, String(u.count)]);
                  const csv = ['user,count', ...rows.map(r => r.join(','))].join('\n');
                  const blob = new Blob([csv], { type: 'text/csv' });
                  const url = URL.createObjectURL(blob);
                  const a = document.createElement('a');
                  a.href = url; a.download = 'top_users.csv'; a.click(); URL.revokeObjectURL(url);
                }}
              >Export CSV</button>
            </div>
            <div className="mt-1 divide-y divide-divider/80">
              {(topUsers.slice(0,5).length ? topUsers.slice(0,5) : [{user:'192.168.0.103',count:23},{user:'192.168.1.8',count:12}]).map((u) => (
                <div key={u.user} className="flex items-center justify-between py-2 text-sm">
                  <span className="text-bright/90">{u.user}</span>
                  <span className="text-dim">{formatNum(u.count)} msgs</span>
                </div>
              ))}
            </div>
          </div>
        </div>

        {/* Largest Files */}
        <div className="grid grid-cols-1 gap-4">
          <div className="rounded-2xl border border-divider bg-panel/80 p-4">
            <div className="text-sm font-semibold text-bright mb-1">Largest files</div>
            <div className="text-xs text-dim mb-3">Top 10 by size · from uploads</div>
            <div className="flex justify-end mb-2">
              <button
                className="text-[11px] px-2 py-1 border border-divider rounded bg-panel hover:bg-panel/80"
                onClick={() => {
                  const rows = (largestFiles.length ? largestFiles : []).map(f => [f.filename, formatBytes(f.bytes as any as number), f.file_type, f.uploader_ip]);
                  const csv = ['filename,size,file_type,uploader_ip', ...rows.map(r => r.join(','))].join('\n');
                  const blob = new Blob([csv], { type: 'text/csv' });
                  const url = URL.createObjectURL(blob);
                  const a = document.createElement('a');
                  a.href = url; a.download = 'largest_files.csv'; a.click(); URL.revokeObjectURL(url);
                }}
              >Export CSV</button>
            </div>
            <div className="mt-1 divide-y divide-divider/80">
              {(largestFiles.length ? largestFiles : [{filename:'Sample.pdf',bytes:1_234_567,uploader_ip:'192.168.0.103',file_type:'application/pdf'}]).map((f, idx) => (
                <div key={`${f.filename}-${idx}`} className="flex items-center justify-between py-2 text-sm">
                  <div className="min-w-0">
                    <div className="truncate text-bright/90" title={f.filename}>{f.filename}</div>
                    <div className="text-[11px] text-dim">{f.file_type} · {f.uploader_ip}</div>
                  </div>
                  <span className="text-dim">{formatBytes(f.bytes)}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
