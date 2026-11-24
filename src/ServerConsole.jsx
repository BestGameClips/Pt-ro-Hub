import React,{useState,useEffect,useRef} from 'react';
import {invoke,Channel} from '@tauri-apps/api/core';
export default function ServerConsole({server,onBack,onManage}){
 const [lines,L]=useState([]),[status,S]=useState('Connexion…'),[state,T]=useState('Inconnu'),[stats,M]=useState({}),[command,C]=useState(''),[error,E]=useState(''),[busy,B]=useState(false),[retry,R]=useState(0),[confirm,F]=useState(null),[follow,A]=useState(true);
 const output=useRef(null);
 useEffect(()=>{
  let stopped=false,timer,reconnect;
  function append(text){if(!stopped)L(old=>[...old,String(text).slice(0,16000)].slice(-1500))}
  async function connect(){
   const channel=new Channel();
   channel.onmessage=msg=>{if(stopped)return;const arg=msg.args?.[0];try{
    if(msg.event==='auth success'){S('Connectée');E('')}
    else if(msg.event==='console output'||msg.event==='install output')append(arg??'');
    else if(msg.event==='status')T(String(arg));
    else if(msg.event==='stats'){const v=typeof arg==='string'?JSON.parse(arg):arg;M(v||{});if(v?.state)T(v.state)}
   }catch{E('Message console non traité')}};
   try{await invoke('console_stream',{id:server.id,channel});if(!stopped){S('Reconnexion…');reconnect=setTimeout(connect,1000)}}catch(e){if(!stopped){S('Déconnectée');E(String(e))}}
  }
  async function resources(){try{const r=await invoke('resources',{id:server.id});if(!stopped){T(r.attributes?.current_state||'Inconnu');M(r.attributes?.resources||{})}}catch{}}
  L([]);E('');S('Connexion…');connect();resources();timer=setInterval(resources,10000);
  return()=>{stopped=true;clearInterval(timer);clearTimeout(reconnect)};
 },[server.id,retry]);
 useEffect(()=>{if(follow&&output.current)output.current.scrollTop=output.current.scrollHeight},[lines,follow]);
 async function action(signal){B(true);E('');try{await invoke('power',{id:server.id,signal});F(null)}catch(e){E(String(e))}finally{B(false)}}
 async function send(e){e.preventDefault();if(!command.trim()||busy)return;B(true);E('');try{await invoke('console_command',{id:server.id,command});C('')}catch(e){E(String(e))}finally{B(false)}}
 const mb=n=>Number.isFinite(n)?(n/1048576).toFixed(1)+' MiB':'—';
 return <section className="serverview"><div className="heading"><div><button onClick={onBack} disabled={busy}>← Retour aux serveurs</button><h1>{server.name}</h1><p>État du serveur : {state}</p></div><div className="actions"><button disabled={busy} className="startbtn" onClick={()=>action('start')}>▶ Start</button><button disabled={busy} onClick={()=>F('restart')}>↻ Restart</button><button disabled={busy} className="danger" onClick={()=>F('stop')}>■ Stop</button></div></div><nav className="servernav"><button className="navactive">Console</button>{Object.entries({files:"Fichiers",backups:"Sauvegardes",schedules:"Planifications",databases:"Bases SQL",allocations:"Réseau",subusers:"Utilisateurs"}).map(([id,label])=><button key={id} disabled={busy} onClick={()=>onManage(id)}>{label}</button>)}</nav><div className="cards">{[['CPU',Number.isFinite(stats.cpu_absolute)?stats.cpu_absolute.toFixed(1)+' %':'—'],['RAM',mb(stats.memory_bytes)],['Disque',mb(stats.disk_bytes)]].map(([k,v])=><article className="panel metric" key={k}><small>{k}</small><strong>{v}</strong></article>)}</div>{error&&<p role="alert" className="alert error">{error}</p>}<section className="panel terminal"><div className="tablehead"><div><h2>Console en direct</h2><small>{status}</small></div><div className="actions"><button onClick={()=>R(x=>x+1)}>Reconnecter</button><button onClick={()=>L([])}>Effacer</button><button onClick={()=>A(x=>!x)}>Suivi : {follow?'ON':'OFF'}</button></div></div><div className="terminaloutput" ref={output} aria-label="Logs du serveur">{lines.length?lines.map((line,i)=><div key={i}>{line.replace(/\x1b\[[0-?]*[ -/]*[@-~]/g,'')}</div>):<div className="muted">En attente des logs du serveur…</div>}</div><form className="commandbar" onSubmit={send}><input value={command} onChange={e=>C(e.target.value)} placeholder="Entrer une commande…" aria-label="Commande serveur" maxLength={4096}/><button disabled={busy||!command.trim()} type="submit">Envoyer</button></form></section>{confirm&&<div className="overlay"><section className="panel dialog" role="dialog" aria-label="Confirmer"><h2>Confirmer {confirm==='stop'?'l’arrêt':'le redémarrage'}</h2><p>Cette action peut interrompre les joueurs connectés.</p><div className="actions"><button disabled={busy} onClick={()=>F(null)}>Annuler</button><button disabled={busy} className="danger" onClick={()=>action(confirm)}>Confirmer</button></div>{error&&<p role="alert">{error}</p>}</section></div>}</section>
}
