import type {SwitchShortcut} from './types';

// Physical KeyboardEvent.code -> the USB HID usages used by both native adapters.
const codes:Record<string,number> = {
 ControlLeft:224,ControlRight:224,ShiftLeft:225,ShiftRight:225,AltLeft:226,AltRight:226,MetaLeft:227,MetaRight:227,
 Enter:40,Escape:41,Backspace:42,Tab:43,Space:44,Minus:45,Equal:46,BracketLeft:47,BracketRight:48,Backslash:49,
 Semicolon:51,Quote:52,Backquote:53,Comma:54,Period:55,Slash:56,CapsLock:57,ScrollLock:71,
 Insert:73,Home:74,PageUp:75,Delete:76,End:77,PageDown:78,ArrowRight:79,ArrowLeft:80,ArrowDown:81,ArrowUp:82,
 NumLock:83,NumpadDivide:84,NumpadMultiply:85,NumpadSubtract:86,NumpadAdd:87,NumpadEnter:88,NumpadDecimal:99,IntlBackslash:100,ContextMenu:101,
};
for(let i=0;i<26;i++)codes[`Key${String.fromCharCode(65+i)}`]=4+i;
for(let i=1;i<=9;i++){codes[`Digit${i}`]=29+i;codes[`Numpad${i}`]=88+i;}
codes.Digit0=39;codes.Numpad0=98;
for(let i=1;i<=12;i++)codes[`F${i}`]=57+i;
const labels:Record<number,string>={40:'Enter',41:'Esc',42:'Backspace',43:'Tab',44:'Space',45:'−',46:'=',47:'[',48:']',49:'\\',51:';',52:"'",53:'`',54:',',55:'.',56:'/',57:'Caps Lock',71:'Scroll Lock',73:'Insert',74:'Home',75:'Page Up',76:'Delete',77:'End',78:'Page Down',79:'→',80:'←',81:'↓',82:'↑',83:'Num Lock',84:'小键盘 /',85:'小键盘 *',86:'小键盘 −',87:'小键盘 +',88:'小键盘 Enter',98:'小键盘 0',99:'小键盘 .',100:'\\',101:'菜单',224:'Ctrl',225:'Shift',226:'Alt / Option',227:'Win / Command'};
export const keyFromCode = (code:string) => codes[code];
export const canonicalKey = (key:number) => key>=228&&key<=231?key-4:key;
export function shortcutKeyLabel(raw:number) {
 const key=canonicalKey(raw);
 if(key>=4&&key<=29)return String.fromCharCode(65+key-4);
 if(key>=30&&key<=38)return String(key-29);
 if(key===39)return '0';
 if(key>=58&&key<=69)return `F${key-57}`;
 if(key>=89&&key<=97)return `小键盘 ${key-88}`;
 return labels[key]??`键 ${key}`;
}
export function shortcutKeys(value:SwitchShortcut):number[] {
 return value==='Ctrl'?[224]:value==='Shift'?[225]:value==='None'?[]:value.Keys;
}
export function shortcutLabel(value:SwitchShortcut) {
 return value==='None'?'无需按键':shortcutKeys(value).map(shortcutKeyLabel).join(' + ');
}
export function shortcutError(value:SwitchShortcut,emergencyKey:number):string|null {
 if(['Ctrl','Shift','None'].includes(value as string))return null;
 if(!value||typeof value!=='object'||!Array.isArray(value.Keys))return '请选择切换按键。';
 const keys=value.Keys.map(canonicalKey);
 if(!keys.length||keys.length>6||new Set(keys).size!==keys.length||keys.some(k=>!Object.values(codes).includes(k)))return '请选择 1–6 个不同的常规键盘按键。';
 if(keys.includes(224)&&keys.includes(226)&&keys.includes(emergencyKey))return '不能与紧急释放快捷键重合，请换一组按键。';
 return null;
}
