import type { Status, Topology, Monitor } from './types';
import type { Platform } from './presentation';
const monitor=(device:string,id:string,name:string,x:number,y:number,width:number,height:number,scale=1,extra:Partial<Monitor>={}):Monitor=>({id,device,name,primary:id.endsWith('1'),rect:{x,y,width,height},resolution:[width,height],scale,rotation:height>width?90:0,enabled:true,mirror_of:null,unit:'PhysicalPixels',...extra});
const topology=(id:string,name:string,os:string,online:boolean,monitors:Monitor[]):Topology=>({device:{id,name,os,online,authorized:true,permission:os==='Windows'?'Granted':'Unverified'},version:1,monitors});
export const scenarioNames={workspace:'多设备工作区',solo:'只有本机',paused:'共享暂停',controlling:'正在跨机控制',mirrors:'长名称 / 竖屏 / 镜像',crowded:'多设备极限布局',discovered:'发现设备',listening:'显示配对码',connecting:'正在配对',confirming:'等待本机确认',waiting:'等待对方确认',completed:'已配对但未上线',timeout:'配对超时',permission:'权限不足',observe:'observe-only',unavailable:'后台不可用',loading:'正在读取'} as const;
export type Scenario=keyof typeof scenarioNames;
// These opt-in fixtures never enter the IPC path.
export function makeDemo(scenario:Scenario,platform:Platform):Status|null {
 if(scenario==='unavailable'||scenario==='loading')return null;
 const os=platform==='macos'?'MacOS':platform==='ubuntu'?'Linux':'Windows';
 const a=topology('a',platform==='macos'?'我的 Mac':platform==='ubuntu'?'Ubuntu 工作站':'设计工作站',os,true,[monitor('a','a1','主显示器',0,0,2560,1440,1.5),monitor('a','a2','竖屏显示器',-1080,-320,1080,1920)]);
 const b=topology('b','办公电脑','Windows',true,[monitor('b','b1','DELL U2723QE',0,0,3840,2160,1.5)]);
 const c=topology('c','MacBook Pro','MacOS',false,[monitor('c','c1','内建 Retina 显示屏',0,0,1512,982,2,{resolution:[3024,1964],unit:'DesktopPoints'}),monitor('c','c2','外接显示器',1512,-98,1920,1080,2,{resolution:[3840,2160],unit:'DesktopPoints'})]);
 const s:Status={enabled:true,state:'Demo 示例 · 不建立真实网络连接，不捕获或注入输入',source:null,active:null,identity:'a',topology:[a,b,c],certificate:[],trusted:[{id:'b',name:b.device.name,certificate:[]},{id:'c',name:c.device.name,certificate:[]}],rtt_ms:null,last_error:null,received:0,config:{version:1,sharing:true,port:24872,endpoints:[],links:[],tuning:{mouse_hz:250,motion_wait_ms:0,edge_wait_ms:0,heartbeat_ms:100,lease_ms:750,emergency_key:41}},peer_sharing:{b:false},discovered:[],effective_links:[{from:{device:'a',monitor:'a1',edge:'Right',start:0,end:1},to:{device:'b',monitor:'b1',edge:'Left',start:0,end:1},enabled:true,require_ctrl:true},{from:{device:'b',monitor:'b1',edge:'Left',start:0,end:1},to:{device:'a',monitor:'a1',edge:'Right',start:0,end:1},enabled:true,require_ctrl:true}]};
 s.local_addresses=[{name:'以太网（示例）',address:'192.0.2.10'}];
 if(scenario==='solo'){s.topology=[a];s.trusted=[];s.effective_links=[];s.enabled=false;s.config.sharing=false;}
 if(scenario==='paused'){s.enabled=false;s.config.sharing=false;}
 if(scenario==='controlling'){s.source='a';s.active='b';s.peer_sharing={b:true};}
 if(scenario==='permission'){a.device.permission='Denied';s.enabled=false;s.last_error='Permission denied: input monitoring permission is unavailable';}
 if(scenario==='observe')s.observe_only=true;
 if(scenario==='mirrors'){a.device.name='用于三维建模与实验数据分析的主工作站（超长中文设备名称）';a.monitors[0].name='DELL UltraSharp 32 4K USB-C 显示器 · 办公桌左侧主显示器';a.monitors.push(monitor('a','a3','会议室镜像输出',0,0,2560,1440,1.5,{mirror_of:'a1',primary:false}));}
 if(scenario==='crowded')for(let i=0;i<5;i++)s.topology.push(topology(`extra${i}`,`实验室工作站 ${i+1}`,'Windows',i%2===0,[monitor(`extra${i}`,`extra${i}-1`,'主屏',-3840,-1080,3840,2160),monitor(`extra${i}`,`extra${i}-2`,'竖屏',0,0,1440,2560)]));
 if(scenario==='discovered')s.discovered=[{id:'demo-nearby',name:'待配对的笔记本（示例）',service:'demo-only',addresses:['192.0.2.20']}];
 const phases:Record<string,string>={listening:'listening',connecting:'connecting',confirming:'confirming',waiting:'waiting',completed:'completed',timeout:'error'};
 if(phases[scenario]){s.pairing={phase:phases[scenario],session_id:'demo-session',code:scenario==='listening'?'48261590':null,peer_name:scenario==='listening'?null:'办公电脑',peer_address:'192.0.2.20',expires_at_ms:Date.now()+110000,error:scenario==='timeout'?'deadline has elapsed':null};if(scenario!=='completed')s.enabled=false;if(scenario==='completed'){b.device.online=false;s.peer_sharing={};}}
 return s;
}
