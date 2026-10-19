/// <reference types="vite/client" />
import React, {useEffect,useMemo,useRef,useState} from 'react';
import {createRoot} from 'react-dom/client';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {Monitor,Network,Settings,Pause,Play,Sun,Moon,Plus,Trash2,ChevronRight,HelpCircle,Loader2,FlaskConical,ChevronDown,Keyboard,CheckCircle2,Waypoints,X} from 'lucide-react';
import {Button} from './ui/button';
import {Dialog} from './ui/Dialog';
import {ErrorNotice} from './ui/ErrorNotice';
import {LayoutCanvas,type ScreenSelection} from './WorkspaceCanvas';
import {makeDemo,scenarioNames,type Scenario} from './demoScenarios';
import {controlLabel,emergencyLabel,detectPlatform,type Platform} from './presentation';
import {ConnectionPanel} from './ConnectionPanel';
import {SwitchingEditor} from './SwitchingEditor';
import {SettingsView} from './SettingsView';
import {HelpView} from './HelpView';
import type {Status,Config,Edge,EdgeEnd} from './types';
import {waitForReceipt} from './commandReceipt';
import {receiveConfig,editConfig,isDirty,editableKey,validateConfig,pairingAddressError,type Draft} from './configDraft';
import {version} from '../package.json';
import './styles.css';
function App() {
  const [tab, setTab] = useState("layout");
  const [busyCommand, setBusyCommand] = useState<string | null>(null);
  const [readFailed, setReadFailed] = useState(false);
  const [retainedError, setRetainedError] = useState('');
  const [dismissedError, setDismissedError] = useState('');
  const [readError, setReadError] = useState('');
  const [selected, setSelected] = useState<ScreenSelection>(null);
  const [edgeOpen, setEdgeOpen] = useState(false);
  const [connectOpen, setConnectOpen] = useState(false);
  const [demoControls, setDemoControls] = useState(false);
  const [platform, setPlatform] = useState<Platform>(detectPlatform);
  const [scenario, setScenario] = useState<Scenario>('workspace');
  // Demo is an explicit, session-only Settings action. URL parameters and saved
  // preferences must never substitute fixtures for the initial backend read.
  const [demoMode, setDemo] = useState(false);
  const sample = useMemo(() => demoMode ? makeDemo(scenario, platform) : null, [demoMode, scenario, platform]);
  const [status, setStatus] = useState<Status | null>(null);
  const [error, setError] = useState("");
  const [feedback, setFeedback] = useState("");
  const pendingCommand = useRef<string | null>(null);
  const [dark, setDark] = useState(() => {
    const query = new URLSearchParams(location.search).get('theme');
    if (query) return query !== 'light';
    try { return localStorage.getItem('crossdesk.ui.theme') === 'dark'; } catch { return false; }
  });
  const [debugInfo, setDebugInfo] = useState(() => {
    try { return localStorage.getItem('crossdesk.ui.debug') === 'true'; } catch { return false; }
  });
  useEffect(() => {
    try { localStorage.setItem('crossdesk.ui.debug', String(debugInfo)); } catch { /* Preference remains usable for this session. */ }
  }, [debugInfo]);
  const [draft, setDraft] = useState<Draft | null>(null);
  const draftRef = useRef<Draft | null>(null);
  const saveAttempt = useRef<{id:string;value:Config}|null>(null);
  const configErrors = useRef(new Set<string>());
  const [saveError,setSaveError] = useState('');
  const saveTask = useRef<Promise<void>|null>(null);
  const updateDraft = (next:Draft) => { draftRef.current=next;setDraft(next); };
  const config = draft?.value ?? null;
  const setConfig = (value: Config) => { setFeedback('');setSaveError('');updateDraft(editConfig(draftRef.current,value)); };
  const syncConfig = (value: Config, receipt?:Status['last_command']) => {
    const attempt=saveAttempt.current;
    const own=attempt && (receipt?.id===attempt.id || editableKey(value)===editableKey(attempt.value));
    if(own&&receipt?.error)configErrors.current.add(receipt.error);
    updateDraft(receiveConfig(draftRef.current,value,own&&!receipt?.error?attempt.value:undefined));
    if(own)saveAttempt.current=null;
  };
  const dirty = isDirty(draft);
  const [ip, setIp] = useState("");
  const [peer, setPeer] = useState("");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [fromEdge, setFromEdge] = useState<Edge>("Right");
  const [toEdge, setToEdge] = useState<Edge>("Left");
  const [range, setRange] = useState([0, 1, 0, 1]);
  useEffect(() => {
    if (!isTauri()) {
      return;
    }
    let alive = true;
    let polling = false;
    const poll = async () => {
      if (polling) return;
      polling = true;
      try {
        const s = await invoke<Status>("daemon_status");
        if (alive && s) {
          setStatus(s);
          setReadFailed(false);
          setReadError("");
          syncConfig(s.config,s.last_command);
        }
      } catch (e) {
        if (alive) { setReadError(String(e)); setRetainedError(String(e)); setReadFailed(true); }
      } finally { polling = false; }
    };
    void poll();
    const timer = setInterval(poll, 500);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, []);
  useEffect(() => {
    document.documentElement.dataset.theme = dark ? "dark" : "light";
    try { localStorage.setItem("crossdesk.ui.theme", dark ? "dark" : "light"); } catch { /* Theme remains usable without storage. */ }
  }, [dark]);
  useEffect(() => {
    if (status?.pairing?.phase === "completed") syncConfig(status.config,status.last_command);
  }, [status?.pairing?.phase, status?.pairing?.session_id]);
  const cmd = async (command: string, fields: Record<string, unknown> = {}) => {
    setError("");
    setDismissedError("");
    if (demoMode) {
      setError("演示模式不执行真实网络或输入命令。");
      return;
    }
    if (pendingCommand.current && command !== "pause") {
      setError("上一项操作仍在等待后台确认。请稍候；需要停止时仍可点击暂停共享。");
      return;
    }
    const id = crypto.randomUUID();
    pendingCommand.current = id;
    if(command==='configure')saveAttempt.current={id,value:fields.config as Config};
    setBusyCommand(command);
    const messages: Record<string, string> = {
      pause: "共享已暂停。",
      configure: "设置已保存。",
      trust: "配对已保存。",
      revoke: "已撤销这台电脑的信任。",
      refresh: "显示器已刷新。",
      discover: "正在搜索电脑…",
      pair_open: "配对码已生成，请在另一台电脑输入本机 IP 和这个配对码。",
      pair_connect: "正在连接对方。请查看配对区域，并在两台电脑上确认。",
      pair_confirm: "本机已确认，等待配对完成。",
      pair_cancel: "本次配对已取消。",
      export_diagnostics: "诊断文件已导出。",
      resume: "共享已启用。",
    };
    setFeedback(command==='configure'?'':command === "pause" ? "正在暂停共享并等待后台释放输入…" : "正在处理，请稍候…");
    try {
      await invoke("daemon_command", { message: { command, ...fields, request_id: id } });
      const fresh = await waitForReceipt({id, read:()=>invoke<Status|null>('daemon_status'), current:()=>pendingCommand.current});
      if (!fresh) return;
      setStatus(fresh);
      syncConfig(fresh.config,fresh.last_command);
      if (fresh.last_command?.error) throw Error(fresh.last_command.error);
      if(command==='configure')setSaveError('');
      else setFeedback(command === 'export_diagnostics' && fresh.diagnostics_path ? '诊断已导出：'+fresh.diagnostics_path : messages[command] ?? '后台已完成操作。');
    } catch (e) {
      if (pendingCommand.current === id) {
        setFeedback("");
        setError(String(e));
        if(command==='configure')setSaveError(String(e));
      }
    } finally {
      if(saveAttempt.current?.id===id && pendingCommand.current===id)saveAttempt.current=null;
      if (pendingCommand.current === id) { pendingCommand.current = null; setBusyCommand(null); }
    }
  };
  useEffect(() => {
    const appearance=config?.appearance;
    if (appearance?.dark != null) setDark(appearance.dark);
    if (appearance?.debug_info != null) setDebugInfo(appearance.debug_info);
  }, [config?.appearance?.dark, config?.appearance?.debug_info]);
  const saveAppearance = (key: 'dark' | 'debug_info', value: boolean) => {
    if (demoMode || !status) { if(key==='dark')setDark(value);else setDebugInfo(value); return; }
    const current=draftRef.current?.value??status.config;
    setConfig({...current,appearance:{dark:null,debug_info:null,...current.appearance,[key]:value}});
  };
  const s = demoMode ? sample : status;
  useEffect(() => {
    if (isTauri() && demoMode && status?.enabled) {
      setDemo(false);
      setPlatform(detectPlatform());
      setSelected(null);
      setFeedback('后台已启用真实共享，已退出 Demo 并显示真实状态。');
    }
  }, [demoMode, status?.enabled]);
  const cfg = (demoMode ? sample?.config : config) ?? { version: 1, port: 24872, endpoints: [], links: [] };
  const save = async () => {
    const current=draftRef.current;
    if(!current||!isDirty(current)||pendingCommand.current||demoMode||!status||readFailed)return;
    if(validateConfig(current.value))return;
    const task=cmd('configure',{config:{...current.value,sharing:current.remote.sharing??false}});
    saveTask.current=task;
    await task;
    if(saveTask.current===task)saveTask.current=null;
  };
  const displayedLinks = cfg.links;
  const screens =
    s?.topology.flatMap((t) =>
      t.monitors
        .filter((m) => m.enabled && !m.mirror_of)
        .map((m) => ({
          key: JSON.stringify([t.device.id, m.id]),
          label: `${t.device.name} / ${m.name || m.id}`,
        })),
    ) ?? [];
  const addEdge = () => {
    setError("");
    try {
      if (!from || !to || !screens.some(m => m.key === from) || !screens.some(m => m.key === to)) throw Error("请先选择两台电脑的屏幕。对方上线后才能添加边界。");
      const [fd, fm] = JSON.parse(from);
      const [td, tm] = JSON.parse(to);
      if (!range.every(Number.isFinite) || range[0] < 0 || range[1] > 1 || range[0] >= range[1] || range[2] < 0 || range[3] > 1 || range[2] >= range[3]) throw Error("范围必须在 0%–100% 内，起点要小于终点。");
      if (fd === td)
        throw Error("同一电脑无需切换方向，请在系统设置中调整屏幕。");
      const end = (
        device: string,
        monitor: string,
        edge: Edge,
        start: number,
        end: number,
      ): EdgeEnd => ({ device, monitor, edge, start, end });
      setConfig({
        ...cfg,
        links: [
          ...cfg.links,
          {
            from: end(fd, fm, fromEdge, range[0], range[1]),
            to: end(td, tm, toEdge, range[2], range[3]),
            enabled: true,
            require_ctrl: true,
          },
        ],
      });
    } catch (e) {
      setError(String(e));
    }
  };
  useEffect(() => { if (status?.last_error&&!configErrors.current.has(status.last_error)) setRetainedError(status.last_error); }, [status?.last_error]);
  const backendError=status?.last_error&&!configErrors.current.has(status.last_error)?status.last_error:'';
  const currentError = demoMode ? (scenario==='unavailable' ? 'Demo：后台响应不可用，无法读取设备状态。' : s?.last_error ?? '') : readError || error || backendError || retainedError;
  const pairingBusy = !!s?.pairing && ['listening','connecting','confirming','waiting'].includes(s.pairing.phase);
  const demoBlockedReason = status?.enabled ? '请先暂停真实共享' : busyCommand||dirty ? '请等待设置保存和当前操作完成' : status?.pairing && ['listening','connecting','confirming','waiting'].includes(status.pairing.phase) ? '请先完成或取消真实配对' : isTauri() && (!status || readFailed) ? '尚不能确认后台状态，暂不能进入演示' : '';
  const toggleDemo = () => {
    if (!demoMode && demoBlockedReason) return;
    setDemo(!demoMode);
    setPlatform(detectPlatform());
    setScenario('workspace');
    setSelected(null);
    setConnectOpen(false);
    setEdgeOpen(false);
    setDemoControls(false);
    setTab('layout');
  };
  const settingsBlocked = pairingBusy ? '请先完成或取消当前配对' : readFailed ? '后台状态读取失败，暂不能保存' : '';
  const configKey=editableKey(config);
  useEffect(()=>{
    if(!dirty||demoMode||!status||settingsBlocked||busyCommand||saveError||!config||validateConfig(config))return;
    const timer=setTimeout(()=>void save(),350);
    return ()=>clearTimeout(timer);
  },[configKey,dirty,demoMode,!!status,settingsBlocked,busyCommand,saveError]);
  const editingBusy=!!busyCommand&&busyCommand!=='configure';
  const saveProblem=!demoMode&&config?validateConfig(config):null;
  const saveState=demoMode?'':saveError?'保存失败':saveProblem?'请检查设置':dirty?(settingsBlocked?'等待保存':'正在保存…'):busyCommand==='configure'?'正在保存…':'';
  const unavailableReason = demoMode ? 'Demo 不执行真实共享命令' : readFailed ? '后台已不可用，请完全退出后重新打开' : !status ? '尚未连接原生后台' : '';
  const canPause = !!s?.enabled || pairingBusy || (!!busyCommand&&busyCommand!=='configure');
  const liveLinks = s?.effective_links ?? [];
  const routeName = (end:EdgeEnd) => { const t=s?.topology.find(t=>t.device.id===end.device); const m=t?.monitors.find(m=>m.id===end.monitor); return (t?.device.name??end.device.slice(0,8))+' / '+(m?.name??end.monitor.slice(0,8)); };
  const edgeNames = {Left:'左边',Right:'右边',Top:'上边',Bottom:'下边'};
  const liveRoute = liveLinks.find(l=>l.enabled&&l.from.device===s?.identity); 
  const local = s?.topology.find(t => t.device.id === s.identity);
  const count = s?.topology.reduce((n,t) => n+t.monitors.filter(m=>m.enabled).length,0) ?? 0;
  const navigate = (next:string) => setTab(next);
  const openConnection = () => { setConnectOpen(true); setTab('connections'); };
  const quitApp = async () => {
    // Finish the last queued edit before stopping the daemon. A failed save stays
    // visible; quitting must not silently discard a just-adjusted setting.
    if(saveTask.current)await saveTask.current;
    if(isDirty(draftRef.current))await save();
    if(isDirty(draftRef.current))return '设置尚未保存，请检查设置或重试后退出。';
    try { setFeedback('正在退出并停止共享…');await invoke('quit_app');return null; }
    catch(e) { setError(String(e));setFeedback('');return String(e); }
  };
  return <div className="app" data-platform={platform}>
    <aside className="sidebar">
      <div className="brand" title="CrossDesk"><span className="brand-mark"><Waypoints size={25}/></span><span className="sr-only">CrossDesk</span></div>
      <nav className="primary-nav" aria-label="主导航">{[['layout','工作区',Monitor],['connections','连接',Network],['settings','设置',Settings],['help','帮助',HelpCircle]].map(([id,label,Icon])=><button key={String(id)} className={tab===id?'nav-item selected':'nav-item'} aria-current={tab===id?'page':undefined} onClick={()=>navigate(String(id))}>{React.createElement(Icon as typeof Monitor,{size:18})}<span>{String(label)}</span></button>)}</nav>
      <div className="sidebar-bottom"><button className="theme-toggle" aria-label={dark?"切换浅色":"切换深色"} title={dark?"切换浅色":"切换深色"} disabled={editingBusy} onClick={()=>saveAppearance('dark',!dark)}>{dark?<Sun size={17}/>:<Moon size={17}/>}<span className="sr-only">{dark?'切换浅色':'切换深色'}</span></button>{debugInfo&&<div className="version">{version}</div>}</div>
    </aside>
    <main className="main-shell">
      <header className="app-header"><div className="header-title"><span className="app-wordmark">CrossDesk<span>/</span></span><h1>{{layout:'工作区',connections:'连接',settings:'设置',help:'帮助'}[tab]}</h1>{demoMode&&<span className="demo-badge">DEMO</span>}</div><div className="header-actions">{tab==='layout'&&<Button onClick={openConnection}><Plus size={16}/>连接电脑</Button>}<Button variant="outline" title={unavailableReason||undefined} disabled={demoMode||!status||busyCommand==='pause'||(readFailed&&!canPause)} onClick={()=>cmd(canPause?'pause':'resume')}>{busyCommand==='pause'||busyCommand==='resume'?<Loader2 size={16} className="spin"/>:canPause?<Pause size={16}/>:<Play size={16}/>} {busyCommand==='pause'?'正在暂停…':busyCommand==='resume'?'正在启用…':!s?'共享不可用':pairingBusy?'停止配对并暂停':canPause?'暂停共享':'启用共享'}</Button></div></header>
      <div className="status-bar" role="status"><div><span className={s?.enabled&&!(readFailed&&!demoMode)?'status-dot':'status-dot paused-dot'}/><strong>{readFailed&&!demoMode?'后台不可用':!s?(demoMode&&scenario==='loading'?'正在读取':isTauri()&&!readFailed?'正在读取后台':'未连接后台'):s.observe_only?'连接测试模式':s.enabled?'共享已启用':'共享已暂停'}</strong>{(debugInfo||!!s?.source)&&<><span className="status-divider"/><span className="control-relation">{readFailed&&!demoMode?'状态待更新':s?debugInfo?controlLabel(s):s.active&&s.source!==s.active?controlLabel(s):'控制状态未知':'设备状态待更新'}</span></>}</div>{demoMode&&<button className="text-button" onClick={()=>setDemoControls(!demoControls)} aria-expanded={demoControls}><FlaskConical size={14}/>示例与平台<ChevronDown size={14}/></button>}</div>
      {demoMode&&demoControls&&<div className="demo-controls"><label>示例状态<select aria-label="Demo 状态" value={scenario} onChange={e=>{setScenario(e.target.value as Scenario);setSelected(null);}}>{Object.entries(scenarioNames).map(([id,name])=><option value={id} key={id}>{name}</option>)}</select></label><label>平台界面<select aria-label="预览平台" value={platform} onChange={e=>setPlatform(e.target.value as Platform)}><option value="windows">Windows</option><option value="macos">macOS · 未验证</option><option value="ubuntu">Ubuntu · 后台未接入</option></select></label><p>示例数据 · 不执行网络、配置或输入命令</p></div>}
      {feedback&&!demoMode&&<div className="command-feedback" role="status" aria-live="polite">{busyCommand?<Loader2 size={16} className="spin"/>:<CheckCircle2 size={16}/>}<span>{feedback}</span>{!busyCommand&&<Button variant="ghost" size="icon" aria-label="关闭操作提示" onClick={()=>setFeedback('')}><X size={15}/></Button>}</div>}
      {currentError&&currentError!==dismissedError&&<div className="global-error"><ErrorNotice raw={currentError} onDismiss={()=>setDismissedError(currentError)}/></div>}
      {currentError&&currentError===dismissedError&&<button className="issue-reminder" onClick={()=>setDismissedError('')}>有一条待查看的错误 · 展开详情</button>}
      <div className={'page-content '+(tab==='layout'?'workspace-page':'scroll-page')}>
        <div className="workspace-content" hidden={tab!=='layout'}>
        {debugInfo&&<div className="workspace-toolbar"><div className="workspace-local"><Monitor size={17}/><strong title={local?.device.name}>{local?.device.name??'本机设备待读取'}</strong><span className="local-tag">本机</span><span className="toolbar-count">{s?<>{s.topology.length} 台电脑 · {count} 块屏幕</>:'尚未读取设备'}</span></div></div>}
        {s?<LayoutCanvas links={cfg.links.length?cfg.links:s.effective_links??[]} onChange={links=>setConfig({...cfg,links})} editDisabled={demoMode||!status||editingBusy||!!settingsBlocked||!!s.topology_error} debugInfo={debugInfo} onRefresh={()=>void cmd('refresh')} refreshDisabled={demoMode||!status||!!busyCommand||readFailed} refreshing={busyCommand==='refresh'} onConnect={openConnection} status={s} stale={readFailed&&!demoMode} selected={selected} onSelect={setSelected}/>:<div className="empty-state workspace-empty">{(demoMode&&scenario==='loading')||(isTauri()&&!readFailed)?<Loader2 size={32} className="spin"/>:<Monitor size={34}/>}<h2>{demoMode&&scenario==='loading'?'正在读取显示器':isTauri()&&!readFailed?'正在读取本机设备':readFailed?'无法读取设备状态':'请在桌面程序中打开工作区'}</h2><p>{demoMode?'示例读取状态，不代表真实后台。':isTauri()?'设备与显示屏信息来自原生后台。读取完成后会自动显示。':'浏览器未连接原生后台，无法读取本机显示屏或进行键鼠共享。请运行 CrossDesk 桌面程序。'}</p></div>}

        <button className="edge-disclosure" disabled={!s} onClick={()=>setEdgeOpen(true)}><span><Waypoints size={17}/><strong>切换方向</strong><small>{liveRoute ? `${edgeNames[liveRoute.from.edge]} → ${s?.topology.find(t=>t.device.id===liveRoute.to.device)?.device.name??'对方'} · ${edgeNames[liveRoute.to.edge]}` : s ? `未设置` : '等待连接'}</small></span><ChevronRight size={17}/></button></div>
        <div hidden={tab!=='connections'}><ConnectionPanel debugInfo={debugInfo} status={s} demo={demoMode} unavailable={readFailed&&!demoMode} commandError={error} cmd={cmd} pending={busyCommand} open={connectOpen} setOpen={setConnectOpen} onWorkspace={()=>setTab('layout')}/></div>
        <div hidden={tab!=='settings'}><SettingsView debugInfo={debugInfo} setDebugInfo={v=>saveAppearance('debug_info',v)} onEdges={()=>setEdgeOpen(true)} status={s} config={demoMode&&sample?sample.config:cfg} onChange={setConfig} saveProblem={saveProblem} blocked={settingsBlocked} cmd={cmd} demo={demoMode} toggleDemo={toggleDemo} demoBlockedReason={demoBlockedReason} pending={busyCommand} dark={dark} setDark={v=>saveAppearance('dark',v)} platform={platform} onQuit={quitApp} addresses={<section className="panel">
              <h2>电脑地址</h2>
              <p>
                对方 IP 变更时，在这里更新。
              </p>
              <div className="edge-form">
                <select
                  aria-label="已授权目标"
                  value={peer}
                  onChange={(e) => setPeer(e.target.value)}
                >
                  <option value="">选择已授权目标</option>
                  {s?.trusted.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                    </option>
                  ))}
                </select>
                <input
                  aria-label="目标 IP 和端口"
                  placeholder="192.168.1.20:24872"
                  value={ip}
                  onChange={(e) => setIp(e.target.value)}
                />
                <Button
                  disabled={demoMode || !peer || !!pairingAddressError(ip)} title={pairingAddressError(ip)??undefined}
                  onClick={() =>
                    setConfig({
                      ...cfg,
                      endpoints: [
                        ...cfg.endpoints.filter((e) => e.peer !== peer),
                        { peer, address: ip.trim().includes(':')?ip.trim():ip.trim()+':24872' },
                      ],
                    })
                  }
                >
                  更新地址
                </Button>
              </div>
              {cfg.endpoints.map((e) => (
                <div className="list-row" key={e.peer}>
                  <span>
                    {s?.trusted.find((t) => t.id === e.peer)?.name} ·{" "}
                    {e.address}
                  </span>
                  <Button
                    size="icon"
                    variant="ghost"
                    aria-label="移除连接"
                    onClick={() =>
                      setConfig({
                        ...cfg,
                        endpoints: cfg.endpoints.filter(
                          (p) => p.peer !== e.peer,
                        ),
                      })
                    }
                  >
                    <Trash2 size={15} />
                  </Button>
                </div>
              ))}

            </section>}/></div>
        {tab==='help'&&<HelpView status={s} platform={platform}/>}
      </div>
      <footer className="app-footer">{saveState&&<span className="autosave-status" role="status">{saveState}{saveError&&<button className="text-button" disabled={!!busyCommand||!!settingsBlocked} onClick={()=>{setSaveError('');void save();}}>重试</button>}</span>}{demoMode&&<span><FlaskConical size={14}/>示例数据</span>}<button className="footer-help" onClick={()=>setTab('help')}><Keyboard size={14}/>紧急释放 <kbd>{emergencyLabel(s)}</kbd></button></footer>
    </main>
    <Dialog title="切换方向" open={edgeOpen} onClose={()=>setEdgeOpen(false)} wide>
      {error&&<ErrorNotice raw={error}/>}
      {s&&<SwitchingEditor status={s} links={cfg.links} disabled={demoMode||!status||editingBusy||!!settingsBlocked} onChange={links=>setConfig({...cfg,links})}/>}
      <details className="switching-advanced"><summary>高级：指定屏幕和触发范围</summary><p className="field-note">通常无需修改。0% 到 100% 表示整条边缘；左右边缘从上往下计算，上下边缘从左往右计算。这里添加的是单向规则。</p><fieldset disabled={demoMode||!status||editingBusy||!!settingsBlocked}>
              <div className="edge-form">
                <select
                  aria-label="移出屏幕"
                  value={from}
                  onChange={(e) => setFrom(e.target.value)}
                >
                  <option value="">移出屏幕</option>
                  {screens.map((m) => (
                    <option key={m.key} value={m.key}>
                      {m.label}
                    </option>
                  ))}
                </select>
                <select
                  aria-label="移出边缘"
                  value={fromEdge}
                  onChange={(e) => setFromEdge(e.target.value as Edge)}
                >
                  {(["Left", "Right", "Top", "Bottom"] as Edge[]).map(
                    (v, i) => (
                      <option key={v} value={v}>
                        {["左", "右", "上", "下"][i]}边缘
                      </option>
                    ),
                  )}
                </select>
                <span>→</span>
                <select
                  aria-label="进入屏幕"
                  value={to}
                  onChange={(e) => setTo(e.target.value)}
                >
                  <option value="">进入屏幕</option>
                  {screens.map((m) => (
                    <option key={m.key} value={m.key}>
                      {m.label}
                    </option>
                  ))}
                </select>
                <select
                  aria-label="进入边缘"
                  value={toEdge}
                  onChange={(e) => setToEdge(e.target.value as Edge)}
                >
                  {(["Left", "Right", "Top", "Bottom"] as Edge[]).map(
                    (v, i) => (
                      <option key={v} value={v}>
                        {["左", "右", "上", "下"][i]}边缘
                      </option>
                    ),
                  )}
                </select>
                <Button size="sm" onClick={addEdge} disabled={demoMode || !from || !to}>
                  <Plus size={14} />
                  添加
                </Button>
              </div>
              <div className="ranges">
                {["移出范围起点（%）", "移出范围终点（%）", "进入范围起点（%）", "进入范围终点（%）"].map(
                  (label, i) => (
                    <label key={label}>
                      {label}
                      <input
                        type="number"
                        min="0"
                        max="100"
                        step="5"
                        value={Math.round(range[i]*100)}
                        onChange={(e) =>
                          setRange((r) =>
                            r.map((v, j) =>
                              j === i ? Number(e.target.value)/100 : v,
                            ),
                          )
                        }
                      />
                    </label>
                  ),
                )}
              </div>
              {cfg.links.length===0&&<p className="route-auto-note">使用默认左右往返。</p>}
              {cfg.links.length===0&&<div className="live-routes">{liveLinks.length?liveLinks.map((l,i)=><p key={i}>{routeName(l.from)} · {edgeNames[l.from.edge]} → {routeName(l.to)} · {edgeNames[l.to.edge]}</p>):<p>连接另一台电脑后自动设置左右往返。</p>}</div>}
              {displayedLinks.map((l, i) => (
                <div className="list-row" key={i}>
                  <span>
                    {routeName(l.from)}{" "}
                    · {{ Left: "左边", Right: "右边", Top: "上边", Bottom: "下边" }[l.from.edge]} {Math.round(l.from.start*100)}%–{Math.round(l.from.end*100)}% →{" "}
                    {routeName(l.to)}{" "}
                    · {{ Left: "左边", Right: "右边", Top: "上边", Bottom: "下边" }[l.to.edge]}
                  </span>
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label="删除切换规则"
                    onClick={() =>
                      setConfig({
                        ...cfg,
                        links: displayedLinks.filter((_, j) => i !== j),
                      })
                    }
                  >
                    <Trash2 size={15} />
                  </Button>
                </div>
              ))}
      </fieldset></details>
      <Button variant="ghost" size="sm" disabled={demoMode||!status||editingBusy||cfg.links.length===0} onClick={()=>setConfig({...cfg,links:[]})}>恢复默认左右往返</Button>
      {(demoMode||!status)&&<p className="field-note">{demoMode?'示例模式不能保存设置。':'请在桌面程序中设置。'}</p>}
    </Dialog>
  </div>;
}
const root=createRoot(document.getElementById('root')!);
import.meta.hot?.dispose(()=>root.unmount());
root.render(<React.StrictMode><App/></React.StrictMode>);
