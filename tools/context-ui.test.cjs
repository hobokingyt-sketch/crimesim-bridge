// Exercises the shipped frontend script with a mock DOM and backend. Not native IPC proof.
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const script = fs.readFileSync(path.join(__dirname, '../web/app.js'), 'utf8');
const tick = () => new Promise((resolve) => setImmediate(resolve));
function element(id = '') {
  const children = new Map();
  return { id, value: id === 'contextArea' ? 'foundation' : '', textContent: '', innerHTML: '', disabled: false,
    className: '', title: '', hidden: false, listeners: {}, options: [],
    addEventListener(name, fn) { this.listeners[name] = fn; },
    click() { if (!this.disabled) return this.listeners.click?.(); },
    appendChild(child) { this.options.push(child); if (this.id === 'contextArea' && !this.value) this.value = child.value; },
    replaceChildren() { this.options = []; this.value = ''; },
    querySelector(key) { if (!children.has(key)) children.set(key, element()); return children.get(key); },
    getBoundingClientRect() { return { width: 200, height: 40 }; }
  };
}
async function boot(overrides = {}) {
  const ids = ['createPack','applyUpdate','playCurrent','scanUpdate','rollback','bootstrap','refreshStatus',
    'contextArea','contextTask','contextRequests','contextHint','sourceRevision','playableRevision','godotStatus',
    'runtimeIntegrity','latestUpdate','recoveryStatus','healthPill','validationList','validationErrors',
    'activityTitle','activityDetail','activityPath'];
  const elements = Object.fromEntries(ids.map(id => [id, element(id)]));
  const controls = ids.slice(0,10).map(id => elements[id]);
  const calls = [];
  const defaults = {
    get_status: { project_present:true, source_revision:1, playable_revision:1, pipeline_ready:true, last_validation:null },
    chat_context_options: { memory_origin:'game_authored_registry', modules:[{id:'foundation',title:'Foundation'},{id:'ui',title:'Interface'}] },
    create_chat_pack: { ok:true, title:'Game handoff created', detail:'Snapshot ready', path:'C:\\Downloads\\pack.zip' },
    frontend_ready: undefined
  };
  const invoke = async (name,args) => {
    calls.push({name,args});
    if (Object.hasOwn(overrides,name)) return typeof overrides[name] === 'function' ? overrides[name](args) : overrides[name];
    return defaults[name];
  };
  vm.runInNewContext(script, {
    window: {__TAURI__:{core:{invoke}}}, console,
    document: { getElementById:id => elements[id], querySelectorAll:() => controls, createElement:() => element() },
    requestAnimationFrame:fn => queueMicrotask(fn), getComputedStyle:() => ({visibility:'visible'})
  });
  await tick();await tick();
  return {elements,calls};
}

test('area and task reach the existing command as a structured request', async () => {
  const {elements:e,calls} = await boot();
  e.contextArea.value = 'ui';e.contextTask.value = 'Simplify Hustles';e.createPack.click();await tick();
  const call=calls.find(x=>x.name==='create_chat_pack');
  assert.equal(call.args.request.module_id,'ui');assert.equal(call.args.request.task,'Simplify Hustles');
  assert.equal(call.args.request.file_requests.length,0);assert.equal(e.activityTitle.textContent,'Game handoff created');
});
test('the handoff choice survives refresh', async () => {
  const {elements:e} = await boot();e.contextArea.value='ui';e.refreshStatus.click();await tick();assert.equal(e.contextArea.value,'ui');
});
test('invalid requested-file JSON is rejected without a backend export', async () => {
  const {elements:e,calls} = await boot();e.contextRequests.value='not JSON';e.createPack.click();await tick();
  assert.equal(calls.filter(x=>x.name==='create_chat_pack').length,0);assert.equal(e.activityTitle.textContent,'File request needs attention');
});
test('chat-authored path and expected hash are forwarded unchanged', async () => {
  const {elements:e,calls} = await boot();e.contextRequests.value=JSON.stringify({file_requests:[{path:'assets/ray.png',sha256:'a'.repeat(64)}]});e.createPack.click();await tick();
  assert.equal(calls.find(x=>x.name==='create_chat_pack').args.request.file_requests[0].sha256,'a'.repeat(64));
});
test('pending export disables controls and reenables them after completion', async () => {
  let resolve;const pending=new Promise(r=>{resolve=r;});const {elements:e}=await boot({create_chat_pack:()=>pending});
  e.createPack.click();await tick();assert.equal(e.createPack.disabled,true);assert.equal(e.contextArea.disabled,true);
  resolve({ok:true,title:'Done'});await tick();await tick();assert.equal(e.createPack.disabled,false);assert.equal(e.contextArea.disabled,false);
});
test('backend rejection remains visible and does not strand busy controls', async () => {
  const {elements:e}=await boot({create_chat_pack:()=>{throw new Error('Requested asset changed');}});e.createPack.click();await tick();await tick();
  assert.equal(e.activityTitle.textContent,'Action failed');assert.match(e.activityDetail.textContent,/asset changed/);assert.equal(e.createPack.disabled,false);
});
test('missing or recovery-blocked project cannot export', async () => {
  for(const status of [{project_present:false},{project_present:true,recovery_required:true}]) {
    const {elements:e,calls}=await boot({get_status:status});assert.equal(e.createPack.disabled,true);e.createPack.click();
    assert.equal(calls.filter(x=>x.name==='create_chat_pack').length,0);
  }
});
test('legacy routes are disclosed and native startup handshake is retained', async () => {
  const {elements:e,calls}=await boot({chat_context_options:{memory_origin:'read_only_legacy_routes',modules:[{id:'foundation',title:'Foundation'}]}});
  assert.match(e.contextHint.textContent,/will not rewrite/);assert.equal(calls.find(x=>x.name==='frontend_ready').args.controlsReady,true);
});
