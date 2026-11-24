#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::sync::Mutex;
use serde_json::{Value,json};
use tauri::State;
#[derive(Clone)]
struct Session { base: reqwest::Url, token: String, admin: bool, client: bool, client_token: Option<String> }
#[derive(Default)]
struct App(Mutex<Option<Session>>);
async fn request(s: &Session, route: &str, body: Option<Value>) -> Result<Value,String> {
 let url=s.base.join(route).map_err(|_|"URL invalide")?;
 let client=reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(20)).build().map_err(|_|"Client HTTP indisponible")?;
 let token=if route.starts_with("api/client") {s.client_token.as_deref().unwrap_or(&s.token)}else{&s.token};
 let req=if let Some(b)=body {client.post(url).json(&b)} else {client.get(url)};
 let res=req.bearer_auth(token).header("Accept","application/json").send().await.map_err(|_|"Connexion impossible")?;
 let status=res.status();
 if !status.is_success(){return Err(format!("Erreur HTTP {}",status.as_u16()));}
 if status.as_u16()==204{return Ok(Value::Null);}
 res.json().await.map_err(|_|"Réponse JSON invalide".into())
}
fn session(state:&App)->Result<Session,String>{state.0.lock().map_err(|_|"Session indisponible")?.clone().ok_or("Non connecté".into())}
#[tauri::command]
async fn connect(state:State<'_,App>,url:String,token:String,admin:bool)->Result<Value,String>{
 let mut base=reqwest::Url::parse(url.trim()).map_err(|_|"URL invalide")?;
 if base.scheme()!="https" || base.host_str().is_none() || !base.username().is_empty() || base.password().is_some(){return Err("HTTPS requis, sans identifiants dans l’URL".into());}
 base.set_path("/");base.set_query(None);base.set_fragment(None);
 let mut s=Session{base,token:token.trim().into(),admin,client:!admin,client_token:None};
 if s.token.is_empty(){return Err("Clé API requise".into());}
 let result=request(&s,if admin{"api/application/servers"}else{"api/client"},None).await?;
 if admin && !s.token.starts_with("ptla_"){s.client=request(&s,"api/client",None).await.is_ok();}
 let result=json!({"inventory":result,"client_access":s.client});
 *state.0.lock().map_err(|_|"Session indisponible")?=Some(s);
 Ok(result)
}
#[tauri::command]
fn disconnect(state:State<'_,App>)->Result<(),String>{*state.0.lock().map_err(|_|"Session indisponible")?=None;Ok(())}
#[tauri::command]
async fn inventory(state:State<'_,App>,category:String,page:u32)->Result<Value,String>{
 let s=session(&state)?;
 let path=match (s.admin,category.as_str()){
 (false,"servers")=>"api/client",(true,"servers")=>"api/application/servers",(true,"nodes")=>"api/application/nodes",(true,"users")=>"api/application/users",(true,"locations")=>"api/application/locations",(true,"allocations")=>return Err("Sélection du node requise".into()),_=>return Err("Catégorie indisponible".into())};
 request(&s,&format!("{}?page={}",path,page.max(1)),None).await
}
fn valid_id(id:&str)->bool{!id.is_empty() && id.len()<=64 && id.chars().all(|c|c.is_ascii_alphanumeric()||c=='-')}
#[tauri::command]
async fn resources(state:State<'_,App>,id:String)->Result<Value,String>{
 let s=session(&state)?; if !s.client||!valid_id(&id){return Err("Mode client requis".into());}
 request(&s,&format!("api/client/servers/{id}/resources"),None).await
}
#[tauri::command]
async fn power(state:State<'_,App>,id:String,signal:String)->Result<Value,String>{
 let s=session(&state)?; if !s.client||!valid_id(&id)||!matches!(signal.as_str(),"start"|"stop"|"restart"){return Err("Action invalide".into());}
 request(&s,&format!("api/client/servers/{id}/power"),Some(json!({"signal":signal}))).await
}
#[tauri::command]
async fn server_section(state:State<'_,App>,id:String,section:String,directory:Option<String>)->Result<Value,String>{
 let s=session(&state)?;
 if !s.client||!valid_id(&id){return Err("Mode client requis".into());}
 let suffix=match section.as_str(){"backups"=>"backups","schedules"=>"schedules","databases"=>"databases","allocations"=>"network/allocations","subusers"=>"users","files"=>"files/list",_=>return Err("Section inconnue".into())};
 let mut route=format!("api/client/servers/{id}/{suffix}");
 if section=="files"{
  let dir=directory.unwrap_or_else(||"/".into());
  let query=reqwest::Url::parse("https://localhost/").map_err(|_|"URL invalide")?;
  let mut query=query;
  query.query_pairs_mut().append_pair("directory",&dir);
  if let Some(q)=query.query(){route.push('?');route.push_str(q);}
 }
 let result=request(&s,&route,None).await?;
 // Return only explicitly selected non-secret metadata, never database credentials.
 let fields:&[&str]=match section.as_str(){
 "backups"=>&["uuid","name","bytes","is_successful","is_locked","created_at"],
 "schedules"=>&["id","name","is_active","is_processing","last_run_at","next_run_at"],
 "databases"=>&["id","name","host","connections"],
 "allocations"=>&["id","ip","ip_alias","port","is_default"],
 "subusers"=>&["uuid","username","email"],
 "files"=>&["name","size","is_file","modified_at"],_=>&[]};
 let rows=result.get("data").and_then(Value::as_array).ok_or("Format de réponse inattendu")?;
 let output:Vec<Value>=rows.iter().map(|item|{
 let attrs=item.get("attributes").unwrap_or(item);
 let map:serde_json::Map<String,Value>=fields.iter().filter_map(|k|attrs.get(*k).map(|v|(k.to_string(),v.clone()))).collect();
 Value::Object(map)
 }).collect();
 Ok(json!({"data":output,"meta":result.get("meta")}))
}
#[tauri::command]
async fn console_credentials(state:State<'_,App>,id:String)->Result<Value,String>{
 let s=session(&state)?;
 if !s.client||!valid_id(&id){return Err("Une clé Client API est nécessaire".into());}
 let response=request(&s,&format!("api/client/servers/{id}/websocket"),None).await?;
 let data=response.get("data").ok_or("Réponse WebSocket invalide")?;
 let socket=data.get("socket").and_then(Value::as_str).ok_or("Adresse WebSocket absente")?;
 let url=reqwest::Url::parse(socket).map_err(|_|"Adresse WebSocket invalide")?;
 if url.scheme()!="wss"||url.host_str().is_none()||!url.username().is_empty()||url.password().is_some(){return Err("WebSocket sécurisé requis".into());}
 let token=data.get("token").and_then(Value::as_str).ok_or("Jeton WebSocket absent")?;
 Ok(json!({"socket":socket,"token":token}))
}
#[tauri::command]
async fn console_command(state:State<'_,App>,id:String,command:String)->Result<Value,String>{
 let s=session(&state)?;
 if !s.client||!valid_id(&id)||command.trim().is_empty()||command.len()>4096{return Err("Commande invalide ou mode client requis".into());}
 request(&s,&format!("api/client/servers/{id}/command"),Some(json!({"command":command}))).await
}
#[tauri::command]
async fn enable_client(state:State<'_,App>,token:String)->Result<Value,String>{
 let mut s=session(&state)?;
 if token.trim().is_empty()||token.starts_with("ptla_"){return Err("Une clé Client API est requise".into());}
 // Verify the second credential without replacing the administrative session.
 let mut client=s.clone();client.token=token.trim().into();client.client=true;client.admin=false;
 request(&client,"api/client",None).await?;
 s.client=true;s.client_token=Some(client.token);
 *state.0.lock().map_err(|_|"Session indisponible")?=Some(s);
 Ok(json!({"client_access":true}))
}
#[tauri::command]
async fn console_stream(state:State<'_,App>,id:String,channel:tauri::ipc::Channel<Value>)->Result<(),String>{
 use futures_util::{SinkExt,StreamExt};
 use tokio_tungstenite::tungstenite::{client::IntoClientRequest,Message};
 let s=session(&state)?;
 if !s.client||!valid_id(&id){return Err("Accès client requis".into());}
 let route=format!("api/client/servers/{id}/websocket");
 let response=request(&s,&route,None).await?;
 let data=&response["data"];
 let socket=data["socket"].as_str().ok_or("Adresse Wings absente")?;
 let token=data["token"].as_str().ok_or("Jeton Wings absent")?;
 let url=reqwest::Url::parse(socket).map_err(|_|"Adresse Wings invalide")?;
 if url.scheme()!="wss"||url.host_str().is_none()||!url.username().is_empty()||url.password().is_some(){return Err("WSS sécurisé requis".into());}
 let mut req=socket.into_client_request().map_err(|_|"Requête WebSocket invalide")?;
 req.headers_mut().insert("Origin",s.base.origin().ascii_serialization().parse().map_err(|_|"Origine invalide")?);
 let (mut ws,_)=tokio::time::timeout(std::time::Duration::from_secs(15),tokio_tungstenite::connect_async(req)).await.map_err(|_|"Délai Wings dépassé")?.map_err(|e|match e{tokio_tungstenite::tungstenite::Error::Http(r)=>format!("Wings HTTP {}",r.status()),_=>"Échec TLS ou réseau Wings".into()})?;
 ws.send(Message::Text(json!({"event":"auth","args":[token]}).to_string().into())).await.map_err(|_|"Authentification non envoyée")?;
 // Bound stream lifetime and stop when receiver disappears.
 let until=tokio::time::Instant::now()+std::time::Duration::from_secs(120);
 loop{
 let msg=match tokio::time::timeout_at(until,ws.next()).await{Ok(Some(Ok(m)))=>m,Ok(Some(Err(_)))=>return Err("Connexion Wings interrompue".into()),_=>break};
 match msg{
 Message::Text(t)=>{
 let v:Value=match serde_json::from_str(&t){Ok(v)=>v,Err(_)=>continue};
 match v["event"].as_str().unwrap_or(""){
 "auth success"=>{for event in ["send logs","send stats"]{ws.send(Message::Text(json!({"event":event,"args":[]}).to_string().into())).await.map_err(|_|"Wings déconnecté")?;}}
 "token expiring"=>{let next=request(&s,&route,None).await?;let token=next["data"]["token"].as_str().ok_or("Jeton absent")?;ws.send(Message::Text(json!({"event":"auth","args":[token]}).to_string().into())).await.map_err(|_|"Wings déconnecté")?;continue;}
 "jwt error"|"token expired"=>return Err("Authentification Wings refusée ou expirée".into()),
 "console output"|"install output"|"stats"|"status"=>{},
 _=>continue
 }
 if channel.send(v).is_err(){break;}
 }
 Message::Ping(p)=>{ws.send(Message::Pong(p)).await.map_err(|_|"Wings déconnecté")?;}
 Message::Close(_)=>break,
 _=>{}
 }
 }
 let _=ws.close(None).await;Ok(())
}
fn file_path(path:&str)->bool{path.starts_with('/')&&path.len()<=4096&&!path.contains('\\')&&!path.contains('\0')&&!path.split('/').any(|x|x==".."||x==".")}
fn file_name(name:&str)->bool{!name.is_empty()&&name!="."&&name!=".."&&!name.contains('/')&&!name.contains('\\')&&!name.contains('\0')&&name.len()<=255}
#[tauri::command]
async fn file_action(state:State<'_,App>,id:String,action:String,directory:String,name:String,new_name:Option<String>,content:Option<String>)->Result<Value,String>{
 let s=session(&state)?;
 if !s.client||!valid_id(&id)||!file_path(&directory)||!file_name(&name){return Err("Chemin ou accès invalide".into());}
 let root=directory.trim_end_matches('/');
 let full=format!("{root}/{name}");
 let prefix=format!("api/client/servers/{id}/files/");
 let client=reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(60)).build().map_err(|_|"Client indisponible")?;
 let token=s.client_token.as_deref().unwrap_or(&s.token);
 let endpoint=|suffix:&str|s.base.join(&format!("{prefix}{suffix}")).map_err(|_|"URL invalide");
 let req=match action.as_str(){
 "read"=>client.get(endpoint("contents")?).query(&[("file",&full)]),
 "write"=>{let body=content.ok_or("Contenu absent")?;if body.len()>2*1024*1024{return Err("Éditeur limité à 2 Mio".into());}client.post(endpoint("write")?).query(&[("file",&full)]).header("Content-Type","text/plain").body(body)},
 "mkdir"=>client.post(endpoint("create-folder")?).json(&json!({"root":directory,"name":name})),
 "rename"=>{let target=new_name.ok_or("Nouveau nom absent")?;if !file_name(&target){return Err("Nom invalide".into());}client.put(endpoint("rename")?).json(&json!({"root":directory,"files":[{"from":name,"to":target}]}))},
 "delete"=>client.post(endpoint("delete")?).json(&json!({"root":directory,"files":[name]})),
 "copy"=>client.post(endpoint("copy")?).json(&json!({"location":full})),
 "compress"=>client.post(endpoint("compress")?).json(&json!({"root":directory,"files":[name]})),
 "decompress"=>client.post(endpoint("decompress")?).json(&json!({"root":directory,"file":name})),
 _=>return Err("Action inconnue".into())};
 let mut res=req.bearer_auth(token).header("Accept",if action=="read"{"text/plain"}else{"application/json"}).send().await.map_err(|_|"Requête fichier impossible")?;
 if !res.status().is_success(){return Err(format!("Fichiers HTTP {}",res.status().as_u16()));}
 if action=="read"{let mut bytes=Vec::new();while let Some(chunk)=res.chunk().await.map_err(|_|"Lecture interrompue")?{if bytes.len()+chunk.len()>2*1024*1024{return Err("Fichier trop volumineux pour l’éditeur".into());}bytes.extend_from_slice(&chunk);}let text=String::from_utf8(bytes).map_err(|_|"Fichier non UTF-8")?;return Ok(json!({"text":text}));}
 Ok(json!({"ok":true}))
}
#[tauri::command]
async fn file_transfer(state:State<'_,App>,id:String,directory:String,name:String,bytes:Option<Vec<u8>>)->Result<Value,String>{
 let s=session(&state)?;
 if !s.client||!valid_id(&id)||!file_path(&directory)||!file_name(&name){return Err("Chemin ou accès invalide".into());}
 let upload=bytes.is_some();
 let mut route=s.base.join(&format!("api/client/servers/{id}/files/{}",if upload{"upload"}else{"download"})).map_err(|_|"URL invalide")?;
 if !upload{route.query_pairs_mut().append_pair("file",&format!("{}/{}",directory.trim_end_matches('/'),name));}
 let relative=route.path().trim_start_matches('/').to_owned()+&route.query().map(|q|format!("?{q}")).unwrap_or_default();
 let result=request(&s,&relative,None).await?;
 let signed=result["attributes"]["url"].as_str().ok_or("Lien de transfert absent")?;
 let url=reqwest::Url::parse(signed).map_err(|_|"Lien invalide")?;
 if url.scheme()!="https"||url.host_str().is_none()||!url.username().is_empty()||url.password().is_some(){return Err("Transfert HTTPS requis".into());}
 let client=reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(120)).build().map_err(|_|"Client indisponible")?;
 let mut res=if let Some(bytes)=bytes{if bytes.len()>20*1024*1024{return Err("Upload limité à 20 Mio".into());}let part=reqwest::multipart::Part::bytes(bytes).file_name(name);client.post(url).query(&[("directory",directory)]).multipart(reqwest::multipart::Form::new().part("files",part)).send().await}else{client.get(url).send().await}.map_err(|_|"Transfert impossible")?;
 if !res.status().is_success(){return Err(format!("Transfert HTTP {}",res.status().as_u16()));}
 if upload{return Ok(json!({"ok":true}));}
 let mut data=Vec::new();while let Some(chunk)=res.chunk().await.map_err(|_|"Téléchargement interrompu")?{if data.len()+chunk.len()>20*1024*1024{return Err("Téléchargement limité à 20 Mio".into());}data.extend_from_slice(&chunk);}
 Ok(json!({"bytes":data}))
}
fn main(){tauri::Builder::default().manage(App::default()).invoke_handler(tauri::generate_handler![connect,disconnect,inventory,resources,power,server_section,console_credentials,console_command,enable_client,console_stream,file_action,file_transfer]).run(tauri::generate_context!()).expect("PteroHub startup failed");}
