#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::sync::Mutex;
use serde_json::{Value,json};
use tauri::State;
#[derive(Clone)]
struct Session { base: reqwest::Url, token: String, admin: bool }
#[derive(Default)]
struct App(Mutex<Option<Session>>);
async fn request(s: &Session, route: &str, body: Option<Value>) -> Result<Value,String> {
 let url=s.base.join(route).map_err(|_|"URL invalide")?;
 let client=reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(20)).build().map_err(|_|"Client HTTP indisponible")?;
 let req=if let Some(b)=body {client.post(url).json(&b)} else {client.get(url)};
 let res=req.bearer_auth(&s.token).header("Accept","application/json").send().await.map_err(|_|"Connexion impossible")?;
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
 let s=Session{base,token:token.trim().into(),admin};
 let result=request(&s,if admin{"api/application/servers"}else{"api/client"},None).await?;
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
 let s=session(&state)?; if s.admin||!valid_id(&id){return Err("Mode client requis".into());}
 request(&s,&format!("api/client/servers/{id}/resources"),None).await
}
#[tauri::command]
async fn power(state:State<'_,App>,id:String,signal:String)->Result<Value,String>{
 let s=session(&state)?; if s.admin||!valid_id(&id)||!matches!(signal.as_str(),"start"|"stop"|"restart"){return Err("Action invalide".into());}
 request(&s,&format!("api/client/servers/{id}/power"),Some(json!({"signal":signal}))).await
}
fn main(){tauri::Builder::default().manage(App::default()).invoke_handler(tauri::generate_handler![connect,disconnect,inventory,resources,power]).run(tauri::generate_context!()).expect("PteroHub startup failed");}
