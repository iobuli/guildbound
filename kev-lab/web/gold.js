'use strict';
const $ = id => document.getElementById(id);
const fmt = (x, digits = 2) => x === null || x === undefined ? 'unknown' : Number(x).toLocaleString('en-GB', {maximumFractionDigits: digits});
let observation, busy = false, selectedReference = null;
const endpoint = cutoff => '/api/gold/nucleus' + (cutoff ? '?asof=' + encodeURIComponent(cutoff) : '');
function metric(parent, label, value, detail, tile = false) {
  const box = document.createElement('div');
  if (tile) box.className = 'tile';
  const title = document.createElement('span'); title.textContent = label;
  const n = document.createElement(tile ? 'strong' : 'b'); n.textContent = value;
  const sub = document.createElement('span'); sub.className = 'small'; sub.textContent = detail;
  box.append(title, n, sub); parent.append(box);
}
function history(rows, events = []) {
  const svg = $('history'); svg.replaceChildren();
  const t = rows.map(r => Date.parse(r.at)), prices = rows.map(r => r.close);
  const left=65, width=1010, span=Math.max(1,t.at(-1)-t[0]);
  const x = i => left + (t[i]-t[0])/span*width;
  const low=Math.min(...prices), high=Math.max(...prices);
  function node(name, attrs, text) {const n=document.createElementNS('http://www.w3.org/2000/svg',name); for(const [k,v] of Object.entries(attrs)) n.setAttribute(k,v); if(text!==undefined)n.textContent=text; svg.append(n);return n;}
  for(const [y,label] of [[20,fmt(high)],[140,fmt(low)],[180,'+1'],[225,'0'],[270,'−1']]) {
    node('line',{x1:left,x2:1075,y1:y,y2:y,stroke:'#354039'});node('text',{x:0,y:y+4,fill:'#a3afa7','font-size':12},label);
  }
  const draw=(values,y,color)=>{
    let d=''; values.forEach((v,i)=>{if(v===null)return; const move=i===0||values[i-1]===null||t[i]-t[i-1]>1800000;d+=(move?'M':'L')+x(i)+','+y(v)+' ';});
    node('path',{d,fill:'none',stroke:color,'stroke-width':1.6});
  };
  draw(prices,p=>140-(p-low)/Math.max(.001,high-low)*120,'#e6c77e');
  ['#77cbb5','#8aace5','#ca91ad','#e6c77e'].forEach((color,j)=>draw(rows.map(r=>r.balances[j]),v=>225-v*45,color));
  for(const event of events.filter(e=>e.kind==='msb')) {
    const at=Date.parse(event.observed_at);if(at<t[0]||at>t.at(-1))continue;
    const px=left+(at-t[0])/span*width;
    const marker=node('line',{x1:px,x2:px,y1:20,y2:140,stroke:'#ca91ad','stroke-dasharray':'3 4',opacity:.65});
    const title=document.createElementNS('http://www.w3.org/2000/svg','title');title.textContent=event.tf+' MSB '+event.direction+' at '+event.observed_at;marker.append(title);
  }
  if(selectedReference && observation) {
    const ref=observation.price_references.references.find(r=>r.id===selectedReference);
    if(ref && ref.price>=low && ref.price<=high) {
      const start=Math.max(t[0],Date.parse(ref.known_at));
      if(start<=t.at(-1)){
        const y=140-(ref.price-low)/Math.max(.001,high-low)*120;
        const fixed=node('line',{x1:left+(start-t[0])/span*width,x2:1075,y1:y,y2:y,stroke:'#77cbb5','stroke-width':2,'stroke-dasharray':'6 3'});
        const title=document.createElementNS('http://www.w3.org/2000/svg','title');title.textContent='Fixed '+ref.id+' from '+ref.known_at;fixed.append(title);
      }
    }
  }
  $('rangeLabel').textContent=rows[0].at+' → '+rows.at(-1).at+' · '+rows.length+' observed closes';
  $('scrub').max=rows.length-1; $('scrub').value=rows.length-1;
}
function render(r) {
  observation=r;
  $('asof').value=r.asof.slice(0,16);
  $('download').href=endpoint(r.asof);
  $('status').textContent='Observed through '+r.asof+' · '+(r.valid?'all twelve lines and historical comparisons available':'warming up: some historical comparisons remain unknown');
  $('tiles').replaceChildren();
  metric($('tiles'),'Actual M30 observations',fmt(r.observed_bars,0),'since '+r.first_open.slice(0,10),true);
  metric($('tiles'),'H/L/C values consumed',fmt(r.hlc_values_seen,0),'3 price values per observed bar',true);
  metric($('tiles'),'Observed / elapsed hours',fmt(r.observed_hours,0)+' / '+fmt(r.elapsed_hours,0),fmt(r.unobserved_half_hour_slots,0)+' absent half-hour slots, including market closures',true);
  metric($('tiles'),'Nucleus representation',r.line_count+' lines',r.representation.line_scalar_slots+' line readings + timeframe summaries',true);
  $('flow').textContent=fmt(r.observed_bars,0)+' observed bars → running high/low/close → 30m, 1h, 4h, UTC 1D buckets → 9/26/52-bar ranges → twelve line readings → cross-timeframe states and their history.';
  $('states').replaceChildren();
  for(const state of r.timeframe_states) metric($('states'),state.tf,state.order,'cloud '+fmt(state.cloud,0)+' · balance '+fmt(state.balance));
  $('lines').replaceChildren();
  for(const l of r.lines) {
    const tr=document.createElement('tr');
    for(const text of [l.tf+' · '+l.n,l.observed_tf_bars+'/'+l.n,fmt(l.input_bars,0),fmt(l.window_hours),fmt(l.mid,3),fmt(l.pos,3),[l.slope,l.shift,l.lag].map(x=>x===null?'?':x).join(' / '),fmt(l.high_age_hours)+' / '+fmt(l.low_age_hours),fmt(l.mid_unchanged_hours),fmt(l.mid_unchanged_observations,0)]) {
      const td=document.createElement('td');td.textContent=text;tr.append(td);
    } $('lines').append(tr);
  }
  $('stats').replaceChildren();const s=r.temporal_window.stats;
  if(s) {
    metric($('stats'),'State variety',fmt(s.unique_states,0),fmt(s.samples,0)+' observed states');
    metric($('stats'),'State entropy',fmt(s.state_entropy_bits)+' bits','joint four-timeframe order');
    metric($('stats'),'Conditional entropy',fmt(s.conditional_entropy_bits)+' bits',fmt(s.adjacent_transitions,0)+' adjacent M30 pairs');
    metric($('stats'),'Current state surprise',fmt(s.current_surprise_bits)+' bits',fmt(s.current_prior_occurrences,0)+' earlier occurrences');
    metric($('stats'),'Current state run',fmt(s.current_run_hours)+' h',fmt(s.current_run_observations,0)+' consecutive observations, within this window');
    metric($('stats'),'State change frequency',s.changed_fraction===null?'unknown':fmt(s.changed_fraction*100)+'%','adjacent observed pairs only');
  } else $('stats').textContent='Not enough history for a known four-timeframe order state.';
  $('method').replaceChildren(); for(const text of r.method){const li=document.createElement('li');li.textContent=text;$('method').append(li);}
  renderReferences(r);
  history(r.timeline, r.price_references.events);
}
function cells(parent, values) {
  const tr=document.createElement('tr');
  for(const value of values){const td=document.createElement('td');if(value instanceof Node)td.append(value);else td.textContent=value;tr.append(td);}
  parent.append(tr);
}
function renderReferences(r) {
  const p=r.price_references; selectedReference=null;
  $('referenceDetail').textContent='No level selected.';
  $('referenceSummary').replaceChildren();
  metric($('referenceSummary'),'Constant prices',fmt(p.reference_count,0),'deduplicated at the gold tick');
  metric($('referenceSummary'),'Frozen range versions',fmt(p.anchor_count,0),'previous versions retained within this replay');
  metric($('referenceSummary'),'New contact episodes',fmt(p.event_counts.contact||0,0),'consecutive contact bars count once');
  metric($('referenceSummary'),'Observed MSBs',fmt(p.event_counts.msb||0,0),'close-confirmed structural events');
  const anchors=new Map(p.anchors.map(a=>[a.id,a]));
  const instances=new Map(p.observations.at(-1).instances.map(i=>[i.anchor_id,i]));
  $('anchors').replaceChildren();
  for(const id of p.current_anchor_ids) {
    const a=anchors.get(id), state=instances.get(id);
    cells($('anchors'),[a.tf+' · '+a.nnn.join('–'),a.known_at,fmt(a.low,3),fmt(a.mid,4),fmt(a.high,3),fmt(state?.fib_position,3),a.id]);
  }
  $('levels').replaceChildren();
  const nearest=[...p.references].sort((a,b)=>Math.abs(a.price-r.close)-Math.abs(b.price-r.close)).slice(0,20).sort((a,b)=>b.price-a.price);
  for(const ref of nearest){
    const button=document.createElement('button');button.type='button';button.textContent=fmt(ref.price,3);
    button.addEventListener('click',()=>{selectedReference=ref.id;$('referenceDetail').textContent=ref.id+' · first known '+ref.known_at+' · '+ref.origins.slice(0,8).map(o=>o.tf+(o.n?' / '+o.n+' / fib '+o.ratio:' / confirmed pivot')).join('; ')+(ref.origins.length>8?' … full origins in export':'');renderEvents();history(observation.timeline,observation.price_references.events);});
    cells($('levels'),[button,[...new Set(ref.origins.map(o=>o.tf))].join(', '),ref.origins.length,ref.contacts,ref.last_contact_at||'none observed']);
  }
  $('eventScope').textContent=p.scope.from+' → '+p.scope.to+' · '+p.scope.note;
  renderEvents();
}
function renderEvents() {
  if(!observation)return;
  const p=observation.price_references, tf=$('eventTf').value, kind=$('eventKind').value;
  const rows=p.events.filter(e=>(!tf||e.tf===tf||e.timeframes?.includes(tf))&&(!kind||e.kind===kind)&&(!selectedReference||e.reference_id===selectedReference));
  $('events').replaceChildren();
  const contexts=new Map(p.observations.map(o=>[o.id,o]));
  $('eventCount').textContent='Showing latest '+Math.min(60,rows.length)+' of '+fmt(rows.length,0)+' matching events. Export contains the entire replay and all contemporaneous contexts.';
  for(const e of rows.slice(-60).reverse()){
    const context=contexts.get(e.context_id);
    cells($('events'),[e.observed_at,e.kind,e.tf||e.timeframes.join(', '),fmt(e.price,3),e.direction||(e.kind==='contact'?e.contact_type+' · '+e.episode:'new level'),context?'order ['+context.orders.join(', ')+'] · '+context.instances.length+' instance readings':'unknown']);
  }
}
$('eventTf').addEventListener('change',renderEvents);
$('eventKind').addEventListener('change',renderEvents);
$('clearReference').addEventListener('click',()=>{selectedReference=null;$('referenceDetail').textContent='All price references.';renderEvents();history(observation.timeline,observation.price_references.events);});
async function renderPredictive(cutoff) {
  $('forecastCards').replaceChildren();$('forecastState').textContent='';
  $('forecastStatus').textContent='Fitting the locked policy using only earlier-year labels…';
  try {
    const response=await fetch('/api/gold/predictive'+(cutoff?'?asof='+encodeURIComponent(cutoff):''));
    const r=await response.json();if(!response.ok)throw Error(r.error||response.statusText);
    if(!r.available){$('forecastStatus').textContent=r.reason;return;}
    const labels={direction:'Close higher',large_displacement:'Large displacement',terminal_range_exit:'Close outside prior range'};
    for(const [name,f] of Object.entries(r.forecasts)) metric($('forecastCards'),labels[name],fmt(f.probability*100,1)+'%','earlier-window frequency '+fmt(f.historical_frequency*100,1)+'% · '+f.config);
    const fitted=Object.values(r.forecasts).map(f=>f.fitted_label_through).sort().at(-1);
    $('forecastStatus').textContent='Observed '+r.asof+' · fitted labels through '+fitted+' · experimental; direction and added-structure skill not demonstrated';
    $('forecastState').textContent='12 range instances → '+r.state.unique_ranges+' distinct prior ranges. Fresh breach balance '+fmt(r.state.fresh_breach_balance,2)+'; upward/downward expiry fractions '+fmt(r.state.expiry_up,2)+' / '+fmt(r.state.expiry_down,2)+'. Fixed prior range '+fmt(r.reference.prior_low104,3)+'–'+fmt(r.reference.prior_high104,3)+'. Normal-distance exit baseline '+fmt(r.reference.normal_geometry_exit_baseline*100,1)+'%.';
  } catch(e) {$('forecastStatus').textContent='Forecast unavailable: '+e.message;}
}
async function load(cutoff) {
  if(busy)return; busy=true;
  document.querySelectorAll('button').forEach(b=>b.disabled=true); $('scrub').disabled=true;
  $('status').textContent='Computing from closed observations at or before the cutoff…';
  const pendingForecast=renderPredictive(cutoff);
  try {const response=await fetch(endpoint(cutoff));const r=await response.json();if(!response.ok)throw Error(r.error||response.statusText);render(r);await pendingForecast;}
  catch(e){$('status').textContent='Could not observe: '+e.message;}
  finally{await pendingForecast;busy=false;document.querySelectorAll('button').forEach(b=>b.disabled=false);$('scrub').disabled=false;}
}
$('controls').addEventListener('submit',e=>{e.preventDefault();load($('asof').value+':00Z');});
$('latest').addEventListener('click',()=>load());
$('scrub').addEventListener('input',()=>{if(observation)$('rangeLabel').textContent='Observe at '+observation.timeline[+$('scrub').value].at;});
$('scrub').addEventListener('change',()=>{if(observation)load(observation.timeline[+$('scrub').value].at);});
load();
