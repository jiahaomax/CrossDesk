import type {Status} from './types';

// A transport send only means accepted for delivery, not successful execution.
export async function waitForReceipt({id,read,current,now=Date.now,delay=ms=>new Promise(r=>setTimeout(r,ms)),timeout=15000}: {
  id:string;read:()=>Promise<Status|null>;current:()=>string|null;now?:()=>number;delay?:(ms:number)=>Promise<unknown>;timeout?:number;
}):Promise<Status|null> {
  const deadline=now()+timeout;
  while(now()<deadline) {
    if(current()!==id)return null;
    const status=await read();
    // Pause may supersede this operation while the status request is in flight.
    if(current()!==id)return null;
    if(status?.last_command?.id===id)return status;
    await delay(250);
  }
  throw Error('15 秒内未收到后台确认，不能确认操作成功。请查看后台状态；紧急停止可用已配置的快捷键或托盘中的退出。');
}
