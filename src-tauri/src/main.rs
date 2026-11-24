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
fn main(){tauri::Builder::default().manage(App::default()).invoke_handler(tauri::generate_handler![connect,disconnect,inventory,resources,power,server_section,console_credentials,console_command,enable_client]).run(tauri::generate_context!()).expect("PteroHub startup failed");}
