// Check the real Chrome viewer via its localhost DevTools port; no npm packages.
import {mkdir, writeFile} from 'node:fs/promises';
// A stalled renderer must fail the check rather than leave automation hanging.
const watchdog = setTimeout(() => { console.error('Chrome smoke test timed out'); process.exit(1); }, 90000);
const targets = await (await fetch('http://127.0.0.1:9224/json/list')).json();
const target = targets.find(t => t.url.includes('127.0.0.1:8765'));
if (!target) throw new Error('Open Kev in the Chrome review window first');
const socket = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {socket.onopen=resolve; socket.onerror=reject;});
let id=0;
const pending = new Map();
const errors = [];
socket.onmessage = ({data}) => {
  const msg=JSON.parse(data);
  if (msg.method==='Runtime.exceptionThrown') errors.push(msg.params.exceptionDetails);
  if (!pending.has(msg.id)) return;
  const {resolve,reject}=pending.get(msg.id); pending.delete(msg.id);
  if(msg.error) reject(new Error(JSON.stringify(msg.error))); else resolve(msg.result);
};
function send(method,params={}) {return new Promise((resolve,reject)=>{const next=++id;pending.set(next,{resolve,reject});socket.send(JSON.stringify({id:next,method,params}));});}
async function evaluate(expression) {
  const result=await send('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});
  if(result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
  return result.result.value;
}
await send('Runtime.enable');
await send('Page.reload',{ignoreCache:true});
await new Promise(resolve=>setTimeout(resolve,1000));
await evaluate(`new Promise((resolve,reject)=>{const start=Date.now(); const timer=setInterval(()=>{if(document.getElementById('dbzClip')?.options.length){clearInterval(timer);resolve(true);}else if(Date.now()-start>20000){clearInterval(timer);reject(new Error('catalog timeout'));}},100);})`);
console.log('catalog UI',await evaluate(`({characters:document.getElementById('dbzCharacter').options.length, matches:document.getElementById('dbzClip').options.length})`));
await evaluate(`document.getElementById('dbzLoad').click()`);
const status=await evaluate(`new Promise((resolve,reject)=>{const start=Date.now();const timer=setInterval(()=>{const b=document.getElementById('dbzLoad');const status=document.getElementById('dbzStatus').textContent;if(!b.disabled){clearInterval(timer);resolve(status);}else if(Date.now()-start>60000){clearInterval(timer);reject(new Error('load timeout: '+status));}},100);})`);
if (!/^(Cached|Converted)/.test(status)) throw new Error(status);
await new Promise(resolve=>setTimeout(resolve,500));
console.log('load UI',status);
console.log('telemetry',await (await fetch('http://127.0.0.1:8765/api/state')).json().then(s=>({model:s.model,clip:s.clip,playing:s.playing,duration:s.duration})));
await evaluate(`document.getElementById('dbzRush').click()`);
await evaluate(`new Promise((resolve,reject)=>{const start=Date.now();const timer=setInterval(()=>{if(document.getElementById('clip').value==='DBZ_Rush_01'){clearInterval(timer);resolve(true);}else if(Date.now()-start>20000){clearInterval(timer);reject(new Error('controller rush timeout'));}},100);})`);
await evaluate(`{const clip=document.getElementById('clip');for(let i=0;i<50;i++)clip.dispatchEvent(new Event('change'));}`);
await new Promise(resolve=>setTimeout(resolve,500));
const rush=await evaluate(`({clip:document.getElementById('clip').value,duration:window.ksSeek(0.2)})`);
if(rush.clip!=='DBZ_Rush_01' || Math.abs(rush.duration-0.6)>0.001) throw new Error('controller preview mismatch');
await evaluate(`document.getElementById('play').click()`);
console.log('controller rush', {clip:rush.clip,duration:rush.duration,replays:50});
const shot=await send('Page.captureScreenshot',{format:'png',captureBeyondViewport:false});
await mkdir('staging/kev/sparking',{recursive:true});
await writeFile('staging/kev/sparking/viewer.png',Buffer.from(shot.data,'base64'));
socket.close();
clearTimeout(watchdog);
if(errors.length) throw new Error(JSON.stringify(errors));
