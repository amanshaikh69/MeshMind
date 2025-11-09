// Same-origin proxy to download a peer's file without cross-origin cookies.
// Browser hits our server at /api/peer-file/{ip}/{filename}, we fetch from the peer
// with the internal header to bypass their auth, then return the bytes.
#[get("/peer-file/{ip}/{filename}")]
async fn proxy_peer_file(path: web::Path<(String, String)>) -> Result<HttpResponse, Error> {
    let (ip, filename) = path.into_inner();
    // Build http://{ip}:8080/api/files/{filename} with proper encoding
    let mut url = match reqwest::Url::parse(&format!("http://{}:8080", ip)) {
        Ok(u) => u,
        Err(e) => {
            return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "message": format!("Invalid peer IP/URL: {}", e)
            })));
        }
    };
    {
        let mut segs = url.path_segments_mut().map_err(|_| actix_web::error::ErrorInternalServerError("url"))?;
        segs.push("api");
        segs.push("files");
        segs.push(&filename);
    }
    let client = reqwest::Client::new();
    match client
        .get(url)
        .header("x-peer-llm", "1")
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            let ct = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_string();
            match resp.bytes().await {
                Ok(bytes) => Ok(HttpResponse::build(status)
                    .content_type(ct)
                    .body(bytes)),
                Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                    "success": false,
                    "message": format!("Failed to read peer response: {}", e)
                })) ),
            }
        }
        Err(e) => Ok(HttpResponse::BadGateway().json(serde_json::json!({
            "success": false,
            "message": format!("Failed to fetch from peer {}: {}", ip, e)
        })) ),
    }
}

#[get("/status")]
async fn api_status() -> Result<HttpResponse, Error> {
    let peer_count = CONVERSATION_STORE.get_peer_conversations().await.len();
    let is_llm_host = crate::tcp::is_ollama_available().await;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "peer_count": peer_count,
        "is_llm_host": is_llm_host
    })))
}

// ---------------- P2P HMAC secret management ----------------
async fn get_or_create_hmac_secret() -> std::io::Result<String> {
    if let Ok(from_env) = env::var("P2P_HMAC_SECRET") {
        let v = from_env.trim().to_string();
        if !v.is_empty() {
            return Ok(v);
        }
    }

    let path = "p2p_secret.txt";
    if let Ok(contents) = tokio_fs::read_to_string(path).await {
        let v = contents.trim().to_string();
        if !v.is_empty() {
            return Ok(v);
        }
    }

    // Generate a new secret using hostname + time hashed with SHA-256
    let host = hostname::get().unwrap_or_default();
    let seed = format!("{}:{}:{}", host.to_string_lossy(), chrono::Utc::now().to_rfc3339(), std::process::id());
    let mut hasher = sha2::Sha256::new();
    use sha2::Digest;
    hasher.update(seed.as_bytes());
    let digest = hasher.finalize();
    let secret_hex = hex::encode(digest);

    tokio_fs::write(path, &secret_hex).await?;
    println!("[P2P] Generated HMAC secret and saved to {}: {}", path, secret_hex);
    Ok(secret_hex)
}
mod udp;
mod ip;
mod tcp;
mod llm;
mod conversation;
mod persistence;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::{Mutex as StdMutex, OnceLock};
use actix_web::{get, post, App, HttpResponse, HttpServer, Responder, web, Error};
use actix_web::cookie::{Cookie, SameSite, time::Duration as CookieDuration};
use jsonwebtoken::{encode, decode, EncodingKey, DecodingKey, Header, Validation, Algorithm};
use actix_web::dev::Service;
use std::time::Instant;
use std::env;
use tokio::fs as tokio_fs;
use actix_cors::Cors;
use rust_embed::Embed;
use tokio::sync::Mutex;
use udp::{periodic_broadcast, receive_broadcast};
use tcp::{connect_to_peers, listen_for_connections};
use conversation::CONVERSATION_STORE;
use persistence::{save_uploaded_file, list_uploaded_files, get_file_content, list_received_files, FileInfo, RECEIVED_DIR};
use actix_multipart::Multipart;
use futures_util::TryStreamExt;
use futures_util::future::{Either, ready};
use crate::tcp::{broadcast_file_to_peers, set_p2p_secret, get_announced_files};
use chrono::{Datelike, Duration as ChronoDuration, Utc};

// ---------------- Auth structures ----------------
#[derive(Clone)]
struct NodeAuth { username: String, password: String }

#[derive(serde::Serialize, serde::Deserialize)]
struct Claims { sub: String, exp: usize }

fn load_node_creds() -> NodeAuth {
    // Username
    let username = std::env::var("NODE_USERNAME").ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| {
        // fallback file (sync)
        std::fs::read_to_string("auth_user.txt").unwrap_or_else(|_| "admin".to_string()).trim().to_string()
    });

    // Password
    let password = std::env::var("NODE_PASSWORD").ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| {
        if let Ok(s) = std::fs::read_to_string("auth_secret.txt") { s.trim().to_string() } else { "admin".to_string() }
    });

    NodeAuth { username, password }
}

fn jwt_keys(secret: &str) -> (EncodingKey, DecodingKey) {
    (EncodingKey::from_secret(secret.as_bytes()), DecodingKey::from_secret(secret.as_bytes()))
}

#[derive(serde::Deserialize)]
struct LoginRequest { username: String, password: String }

#[post("/auth/login")]
async fn auth_login(auth: web::Data<NodeAuth>, body: web::Json<LoginRequest>) -> Result<HttpResponse, Error> {
    if body.username != auth.username || body.password != auth.password {
        return Ok(HttpResponse::Unauthorized().json(serde_json::json!({"error":"invalid_credentials"})));
    }
    let exp = (Utc::now() + ChronoDuration::hours(24)).timestamp() as usize;
    let claims = Claims { sub: auth.username.clone(), exp };
    let (ek, _) = jwt_keys(&auth.password);
    let token = encode(&Header::new(Algorithm::HS256), &claims, &ek).map_err(|_| actix_web::error::ErrorInternalServerError("jwt"))?;

    let cookie = Cookie::build("session", token)
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(CookieDuration::hours(24))
        .finish();

    Ok(HttpResponse::Ok().cookie(cookie).json(serde_json::json!({"authenticated": true, "username": auth.username})))
}

#[get("/auth/status")]
async fn auth_status(req: actix_web::HttpRequest, auth: web::Data<NodeAuth>) -> Result<HttpResponse, Error> {
    let cookie = req.cookie("session");
    if let Some(c) = cookie {
        let (_, dk) = jwt_keys(&auth.password);
        if decode::<Claims>(c.value(), &dk, &Validation::new(Algorithm::HS256)).is_ok() {
            return Ok(HttpResponse::Ok().json(serde_json::json!({"authenticated": true, "username": auth.username})));
        }
    }
    Ok(HttpResponse::Ok().json(serde_json::json!({"authenticated": false})))
}

#[post("/auth/logout")]
async fn auth_logout() -> Result<HttpResponse, Error> {
    let cookie = Cookie::build("session", "")
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(CookieDuration::seconds(0))
        .finish();
    Ok(HttpResponse::Ok().cookie(cookie).json(serde_json::json!({"ok": true})))
}

#[derive(Embed)]
#[folder = "./webpage/build/"]
struct WebAssets;

fn send_file_or_default(path: String) -> HttpResponse {
    let path = if path.starts_with("assets/") {
        path
    } else {
        path.trim_start_matches("/app/").to_string()
    };
    
    let asset = WebAssets::get(path.as_str());
    match asset {
        Some(file) => {
            let mime_type = mime_guess::from_path(&path).first_or_octet_stream();
            HttpResponse::Ok()
                .content_type(mime_type.to_string())
                .body(file.data)
        }
        None => {
            let index_asset = WebAssets::get("index.html");
            match index_asset {
                Some(index_file) => {
                    let mime_type = mime_guess::from_path("index.html").first_or_octet_stream();
                    HttpResponse::Ok()
                        .content_type(mime_type.to_string())
                        .body(index_file.data)
                }
                None => HttpResponse::NotFound().body("Not Found"),
            }
        }
    }
}

// ---------------- Performance state and helpers ----------------
#[derive(Default, Clone)]
struct RouteStats {
    durations_ms: Vec<i64>,
    req_count: u64,
    error_count: u64,
}

#[derive(Default, Clone)]
struct TotalsStats {
    durations_ms: Vec<i64>,
    req_count: u64,
    error_count: u64,
}

#[derive(Default)]
struct PerfState {
    per_route: HashMap<String, RouteStats>,
    totals: TotalsStats,
}

fn percentile_ms(xs: &Vec<i64>, p: f64) -> Option<i64> {
    if xs.is_empty() { return None; }
    let mut v = xs.clone();
    v.sort_unstable();
    let idx = (((p / 100.0) * ((v.len() - 1) as f64)).round() as usize).min(v.len() - 1);
    Some(v[idx])
}

#[get("/analytics/engagement")]
async fn analytics_engagement() -> Result<HttpResponse, Error> {
    // Aggregate DAU, WAU, average session duration (10-minute idle) from conversations
    let mut events: Vec<(String, chrono::DateTime<chrono::Utc>)> = Vec::new();

    if let Some(local) = CONVERSATION_STORE.get_local_conversation().await {
        for m in local.messages {
            events.push((m.host_info.ip_address.clone(), m.timestamp));
        }
    }
    let peers = CONVERSATION_STORE.get_peer_conversations().await;
    for (_peer, conv) in peers {
        for m in conv.messages {
            events.push((m.host_info.ip_address.clone(), m.timestamp));
        }
    }

    let now = Utc::now();
    let one_day_ago = now - ChronoDuration::days(1);
    let seven_days_ago = now - ChronoDuration::days(7);

    let mut dau_set: HashMap<String, bool> = HashMap::new();
    let mut wau_set: HashMap<String, bool> = HashMap::new();

    // Group by user
    let mut by_user: HashMap<String, Vec<chrono::DateTime<chrono::Utc>>> = HashMap::new();
    for (user, ts) in events.into_iter() {
        if ts >= one_day_ago { dau_set.insert(user.clone(), true); }
        if ts >= seven_days_ago { wau_set.insert(user.clone(), true); }
        by_user.entry(user).or_default().push(ts);
    }

    // Compute sessions with 10-minute idle threshold
    let idle = ChronoDuration::minutes(10);
    let mut session_durations: Vec<i64> = Vec::new();
    for (_user, mut times) in by_user {
        times.sort();
        if times.is_empty() { continue; }
        let mut start = times[0];
        let mut last = times[0];
        for t in times.into_iter().skip(1) {
            if t - last > idle {
                // close session
                let dur = (last - start).num_seconds().max(0);
                session_durations.push(dur);
                start = t;
            }
            last = t;
        }
        // final session
        let dur = (last - start).num_seconds().max(0);
        session_durations.push(dur);
    }

    let avg_session_seconds = if session_durations.is_empty() {
        0
    } else {
        let sum: i64 = session_durations.iter().sum();
        (sum as f64 / session_durations.len() as f64).round() as i64
    };

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "dau": dau_set.len(),
        "wau": wau_set.len(),
        "avg_session_seconds": avg_session_seconds
    })))
}

#[get("/analytics/perf")]
async fn analytics_perf(state: web::Data<tokio::sync::Mutex<PerfState>>) -> Result<HttpResponse, Error> {
    let state = state.lock().await;

    let mut per_route_vec: Vec<serde_json::Value> = Vec::new();
    for (route, stats) in state.per_route.iter() {
        let p95 = percentile_ms(&stats.durations_ms, 95.0).unwrap_or(0);
        let err_rate = if stats.req_count == 0 { 0.0 } else { stats.error_count as f64 / stats.req_count as f64 };
        per_route_vec.push(serde_json::json!({
            "route": route,
            "p95_ms": p95,
            "error_rate": err_rate
        }));
    }
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "per_route": per_route_vec,
        "totals": {"req_count": state.totals.req_count, "error_count": state.totals.error_count}
    })))
}

#[get("/analytics/network")]
async fn analytics_network(state: web::Data<tokio::sync::Mutex<PerfState>>) -> Result<HttpResponse, Error> {
    let state = state.lock().await;

    let p50 = percentile_ms(&state.totals.durations_ms, 50.0);
    let p95 = percentile_ms(&state.totals.durations_ms, 95.0);
    let p99 = percentile_ms(&state.totals.durations_ms, 99.0);
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "latency_ms": {"p50": p50, "p95": p95, "p99": p99},
        "bandwidth": {"up_bps": serde_json::Value::Null, "down_bps": serde_json::Value::Null}
    })))
}

#[get("/analytics/chat")]
async fn analytics_chat() -> Result<HttpResponse, Error> {
    // Aggregate messages per day and top users from store
    let mut per_day: HashMap<String, usize> = HashMap::new();
    let mut user_counts: HashMap<String, usize> = HashMap::new();

    if let Some(local) = CONVERSATION_STORE.get_local_conversation().await {
        for m in local.messages {
            let ts = m.timestamp;
            let key = format!("{:04}-{:02}-{:02}", ts.year(), ts.month(), ts.day());
            *per_day.entry(key).or_insert(0) += 1;
            let user_key = m.host_info.ip_address.clone();
            *user_counts.entry(user_key).or_insert(0) += 1;
        }
    }

    let peers = CONVERSATION_STORE.get_peer_conversations().await;
    for (_peer, conv) in peers {
        for m in conv.messages {
            let ts = m.timestamp;
            let key = format!("{:04}-{:02}-{:02}", ts.year(), ts.month(), ts.day());
            *per_day.entry(key).or_insert(0) += 1;
            let user_key = m.host_info.ip_address.clone();
            *user_counts.entry(user_key).or_insert(0) += 1;
        }
    }

    // Convert maps to vecs sorted by key/count
    let mut per_day_vec: Vec<(String, usize)> = per_day.into_iter().collect();
    per_day_vec.sort_by(|a, b| a.0.cmp(&b.0));
    let messages_per_day: Vec<serde_json::Value> = per_day_vec
        .into_iter()
        .map(|(date, count)| serde_json::json!({"date": date, "count": count}))
        .collect();

    let mut top_users_vec: Vec<(String, usize)> = user_counts.into_iter().collect();
    top_users_vec.sort_by(|a, b| b.1.cmp(&a.1));
    let top_users: Vec<serde_json::Value> = top_users_vec
        .into_iter()
        .map(|(user, count)| serde_json::json!({"user": user, "count": count}))
        .collect();

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "messages_per_day": messages_per_day,
        "top_users": top_users
    })))
}

#[get("/analytics/files")]
async fn analytics_files() -> Result<HttpResponse, Error> {
    match list_uploaded_files().await {
        Ok(files) => {
            // Aggregate by top-level type (e.g., application, image)
            let mut types: HashMap<String, (u64, u64)> = HashMap::new(); // type -> (count, total_bytes)
            for f in &files {
                let t = f
                    .file_type
                    .split('/')
                    .next()
                    .unwrap_or("other")
                    .to_string();
                let entry = types.entry(t).or_insert((0, 0));
                entry.0 += 1;
                entry.1 += f.file_size as u64;
            }

            let mut types_vec: Vec<serde_json::Value> = Vec::new();
            for (t, (count, total_bytes)) in types.into_iter() {
                types_vec.push(serde_json::json!({
                    "type": t,
                    "count": count,
                    "total_bytes": total_bytes
                }));
            }

            // Largest files (top 10)
            let mut sorted = files.clone();
            sorted.sort_by(|a, b| b.file_size.cmp(&a.file_size));
            let largest: Vec<serde_json::Value> = sorted
                .into_iter()
                .take(10)
                .map(|f| serde_json::json!({
                    "filename": f.filename,
                    "bytes": f.file_size,
                    "uploader_ip": f.uploader_ip,
                    "file_type": f.file_type
                }))
                .collect();

            Ok(HttpResponse::Ok().json(serde_json::json!({
                "types": types_vec,
                "largest": largest
            })))
        }
        Err(e) => Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "success": false,
            "message": e.to_string()
        }))),
    }
}

#[get("/app/")]
async fn get_index() -> impl Responder {
    send_file_or_default("index.html".to_string())
}

#[get("/app/{path:.*}")]
async fn get_root_files(path: actix_web::web::Path<String>) -> impl Responder {
    let path = path.into_inner();
    send_file_or_default(path)
}

#[get("/peers")]
async fn get_peers() -> Result<HttpResponse, actix_web::Error> {
    println!("API: Received request for peer conversations");
    let peer_conversations = CONVERSATION_STORE.get_peer_conversations().await;
    println!("API: Found {} peer conversations", peer_conversations.len());
    for (peer, conv) in &peer_conversations {
        println!("API: Peer {} has {} messages", peer, conv.messages.len());
    }
    Ok(HttpResponse::Ok().json(peer_conversations))
}

#[get("/api/local")]
async fn get_local() -> Result<HttpResponse, actix_web::Error> {
    println!("API: Received request for local conversation");
    let local = CONVERSATION_STORE.get_local_conversation().await;
    match local {
        Some(conv) => Ok(HttpResponse::Ok().json(conv)),
        None => Ok(HttpResponse::Ok().json(serde_json::json!(null))),
    }
}

#[post("/upload")]
async fn upload_file(req: actix_web::HttpRequest, mut payload: Multipart) -> Result<HttpResponse, Error> {
    // Determine client IP: prefer X-Forwarded-For, fallback to peer_addr
    let client_ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next().map(|ip| ip.trim().to_string()))
        .or_else(|| req.peer_addr().map(|sa| sa.ip().to_string()))
        .unwrap_or_else(|| "127.0.0.1".to_string());
    // If loopback, attempt to resolve our LAN IP to be more meaningful in UI
    let client_ip = if client_ip == "127.0.0.1" || client_ip == "::1" {
        std::net::TcpStream::connect("8.8.8.8:53")
            .and_then(|s| s.local_addr())
            .map(|a| a.ip().to_string())
            .unwrap_or(client_ip)
    } else { client_ip };
    
    while let Some(mut field) = payload.try_next().await? {
        if field.name() == "file" {
            let filename = field.content_disposition()
                .get_filename()
                .unwrap_or("unknown")
                .to_string();
            
            let content_type = field.content_type()
                .map(|mime| mime.to_string())
                .unwrap_or_else(|| "application/octet-stream".to_string());
            
            // Collect file data
            let mut file_data = Vec::new();
            while let Some(chunk) = field.try_next().await? {
                file_data.extend_from_slice(&chunk);
            }
            // Enforce 50 MB upload limit
            const MAX_UPLOAD_BYTES: usize = 50 * 1024 * 1024;
            if file_data.len() > MAX_UPLOAD_BYTES {
                println!("API: File too large ({} bytes), rejecting > 50MB", file_data.len());
                return Ok(HttpResponse::PayloadTooLarge().json(serde_json::json!({
                    "success": false,
                    "message": "File exceeds 50MB limit"
                })));
            }
            
            // Save file
            // After save_uploaded_file(...)
            match save_uploaded_file(&filename, &content_type, &file_data, &client_ip).await {
                Ok(file_info) => {
                    println!("API: File uploaded successfully: {}", filename);
                    // Broadcast file to all peers (all types)
                    let _ = broadcast_file_to_peers(filename.clone(), content_type.clone(), file_data.clone()).await;
                    return Ok(HttpResponse::Ok().json(serde_json::json!({
                        "success": true,
                        "message": "File uploaded successfully",
                        "file_info": file_info
                    })));
                }
                Err(e) => {
                    println!("API: File upload failed: {}", e);
                    return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                        "success": false,
                        "message": e.to_string()
                    })));
                }
            }
        }
    }
    
    Ok(HttpResponse::BadRequest().json(serde_json::json!({
        "success": false,
        "message": "No file provided"
    })))
}

#[get("/files")]
async fn get_files() -> Result<HttpResponse, Error> {
    match list_uploaded_files().await {
        Ok(mut files) => {
            // Merge announced peer files (from FILE_META) without duplicates
            let mut set: std::collections::HashSet<(String, String)> = files
                .iter()
                .map(|f| (f.filename.clone(), f.uploader_ip.clone()))
                .collect();
            let local_count = files.len();
            let announced = get_announced_files().await;
            let mut announced_added = 0usize;
            for af in announced {
                let key = (af.filename.clone(), af.uploader_ip.clone());
                if !set.contains(&key) {
                    files.push(af);
                    set.insert(key);
                    announced_added += 1;
                }
            }
            // Also merge in files physically present under received/<peer-ip>/ (peer binaries)
            if let Ok(received) = list_received_files().await {
                let mut received_added = 0usize;
                for rf in received {
                    let key = (rf.filename.clone(), rf.uploader_ip.clone());
                    if !set.contains(&key) {
                        files.push(rf);
                        set.insert(key);
                        received_added += 1;
                    }
                }
                println!("API: Merged {} received files from disk", received_added);
            }
            // Opportunistically fetch remote peer file lists and merge
            if let Ok(mut remote) = fetch_remote_files().await {
                let mut remote_added = 0usize;
                for rf in remote.drain(..) {
                    let key = (rf.filename.clone(), rf.uploader_ip.clone());
                    if !set.contains(&key) {
                        files.push(rf);
                        set.insert(key);
                        remote_added += 1;
                    }
                }
                println!("API: Merged {} files from remote peers", remote_added);
            }
            println!(
                "API: Listed {} files (local={}, announced_added={}, received_added logged above, remote_added logged above)",
                files.len(), local_count, announced_added
            );
            Ok(HttpResponse::Ok().json(files))
        }
        Err(e) => {
            println!("API: Failed to list files: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "message": e.to_string()
            })))
        }
    }
}

// Helper: fetch remote /api/files from known peers (derived from received/<peer-ip>/)
async fn fetch_remote_files() -> Result<Vec<FileInfo>, ()> {
    // --- Simple throttle/cache to avoid spamming peers and logs ---
    struct RemoteCache { last: std::time::Instant, data: Vec<FileInfo>, fetching: bool }
    static REMOTE_CACHE: OnceLock<StdMutex<RemoteCache>> = OnceLock::new();
    let cache = REMOTE_CACHE.get_or_init(|| StdMutex::new(RemoteCache { last: std::time::Instant::now() - std::time::Duration::from_secs(3600), data: Vec::new(), fetching: false }));
    {
        let mut c = cache.lock().unwrap();
        let age = c.last.elapsed();
        if age < std::time::Duration::from_secs(15) || c.fetching {
            // Return cached data to throttle calls
            return Ok(c.data.clone());
        }
        // mark fetching
        c.fetching = true;
    }

    let mut out: Vec<FileInfo> = Vec::new();
    // Build a unique set of peer IPs from received/ and from conversation store
    let mut peer_ips: std::collections::HashSet<String> = std::collections::HashSet::new();
    let base = std::path::Path::new(RECEIVED_DIR);
    if base.exists() {
        if let Ok(mut rd) = tokio::fs::read_dir(base).await {
            while let Ok(Some(entry)) = rd.next_entry().await {
                if let Ok(ft) = entry.file_type().await {
                    if ft.is_dir() {
                        peer_ips.insert(entry.file_name().to_string_lossy().to_string());
                    }
                }
            }
        }
    }
    // Also add peers known from conversations
    let peers_map = CONVERSATION_STORE.get_peer_conversations().await;
    for (peer_ip, _conv) in peers_map.iter() {
        peer_ips.insert(peer_ip.clone());
    }

    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(6))
        .build()
        .map_err(|_| ())?;
    for ip in peer_ips.into_iter() {
        let url = format!("http://{}:8080/api/files", ip);
        println!("API: fetch_remote_files: contacting peer {} at {}", ip, url);
        let mut attempt = 0;
        let max_attempts = 2;
        let mut success = false;
        while attempt < max_attempts {
            attempt += 1;
            let req = client
                .get(&url)
                .header("x-peer-llm", "1")
                .header("Connection", "close");
            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    match resp.json::<Vec<FileInfo>>().await {
                        Ok(mut list) => {
                            let count = list.len();
                            println!(
                                "API: fetch_remote_files: peer {} responded {} with {} files (attempt {})",
                                ip, status, count, attempt
                            );
                            out.append(&mut list);
                            success = true;
                        }
                        Err(e) => {
                            println!(
                                "API: fetch_remote_files: failed to parse JSON from {} (status {}, attempt {}): {}",
                                ip, status, attempt, e
                            );
                        }
                    }
                }
                Err(e) => {
                    println!(
                        "API: fetch_remote_files: error contacting {} (attempt {}): {}",
                        ip, attempt, e
                    );
                }
            }
            if success { break; }
            // simple backoff
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        if !success {
            println!(
                "API: fetch_remote_files: giving up on {} after {} attempts",
                ip, max_attempts
            );
        }
    }
    // update cache
    {
        let mut c = cache.lock().unwrap();
        c.data = out.clone();
        c.last = std::time::Instant::now();
        c.fetching = false;
    }
    Ok(out)
}

#[get("/files/{filename}")]
async fn download_file(path: web::Path<String>) -> Result<HttpResponse, Error> {
    let filename = path.into_inner();
    
    match get_file_content(&filename).await {
        Ok(Some(content)) => {
            // Get file info for content type
            if let Ok(Some(file_info)) = persistence::get_file_info(&filename).await {
                Ok(HttpResponse::Ok()
                    .content_type(file_info.file_type.as_str())
                    .body(content))
            } else {
                Ok(HttpResponse::Ok()
                    .content_type("application/octet-stream")
                    .body(content))
            }
        }
        Ok(None) => {
            Ok(HttpResponse::NotFound().json(serde_json::json!({
                "success": false,
                "message": "File not found"
            })))
        }
        Err(e) => {
            println!("API: Failed to get file {}: {}", filename, e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "message": e.to_string()
            })))
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("[DEBUG] Starting backend...");
    // Initialize conversations directory silently
    if let Err(e) = persistence::init_conversations_dir().await {
        eprintln!("[DEBUG] Error initializing conversations directory: {}", e);
        return Err(e);
    }
    println!("[DEBUG] Conversations directory initialized.");

    // Load saved conversations
    match CONVERSATION_STORE.load_saved_conversations().await {
        Ok(_) => {
            println!("[DEBUG] Saved conversations loaded.");
        },
        Err(e) => {
            eprintln!("[DEBUG] Error loading saved conversations: {:#?}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to load saved conversations: {e}")));
        }
    }

    let received_ips = Arc::new(Mutex::new(HashSet::new()));
    let received_ips_clone = received_ips.clone();

    println!("[DEBUG] Spawning UDP broadcast receiver...");
    // Start UDP broadcast receiver
    tokio::spawn(async move {
        if let Err(e) = receive_broadcast(received_ips_clone).await {
            eprintln!("[DEBUG] Error in UDP receiver task: {}", e);
        }
    });
    
    println!("[DEBUG] Spawning TCP listener...");
    // Start TCP listener
    tokio::spawn(listen_for_connections());

    println!("[DEBUG] Spawning UDP broadcaster...");
    // Start UDP broadcaster
    tokio::spawn(periodic_broadcast());

    println!("[DEBUG] Spawning peer connector...");
    // Start peer connector
    let received_ips_clone = received_ips.clone();
    tokio::spawn(connect_to_peers(received_ips_clone));

    println!("[DEBUG] Opening web browser...");
    // Open web browser silently
    let _ = open::that("http://localhost:8080/app/");
    
    println!("[DEBUG] Starting HTTP server on 0.0.0.0:8080...");
    // Prepare shared state and secrets
    let perf_state = web::Data::new(tokio::sync::Mutex::new(PerfState::default()));
    // Load node auth creds
    let node_auth = load_node_creds();
    let node_auth_data = web::Data::new(node_auth.clone());
    let p2p_secret_string = match get_or_create_hmac_secret().await {
        Ok(s) => s,
        Err(_) => {
            let fallback = "dev-default-secret".to_string();
            println!("[P2P] Failed to load/write secret, using fallback dev secret: {}", fallback);
            fallback
        }
    };
    // Also log when using an env or existing file (masked)
    if env::var("P2P_HMAC_SECRET").is_ok() {
        println!("[P2P] Using HMAC secret from environment.");
    }
    let p2p_secret = web::Data::new(p2p_secret_string.clone());
    // Provide secret to TCP module for HMAC verification/creation
    set_p2p_secret(p2p_secret_string.clone()).await;
    HttpServer::new(move || {
        let perf_state_clone = perf_state.clone();
        let p2p_secret_clone = p2p_secret.clone();
        let node_auth_clone = node_auth_data.clone();
        App::new()
            .app_data(perf_state_clone.clone())
            .app_data(p2p_secret_clone.clone())
            .app_data(node_auth_clone.clone())
            // Auth guard middleware
            .wrap_fn(move |req, srv| {
                let path = req.path().to_string();
                let needs_auth = (path.starts_with("/api/") && !path.starts_with("/api/auth/") && path != "/api/status")
                    || path == "/peers"
                    || path == "/api/local";
                if needs_auth {
                    // Allow internal peer LLM calls: POST /api/chat with header x-peer-llm
                    let is_internal_peer_chat = path == "/api/chat"
                        && req.method() == actix_web::http::Method::POST
                        && req.headers().get("x-peer-llm").map(|v| v == "1" || v == "yes").unwrap_or(false);
                    // Allow internal peer FILE fetches: GET /api/files and /api/files/<name> with header x-peer-llm
                    let is_internal_peer_file = (path == "/api/files" || path.starts_with("/api/files/"))
                        && req.method() == actix_web::http::Method::GET
                        && req.headers().get("x-peer-llm").map(|v| v == "1" || v == "yes").unwrap_or(false);
                    // Allow internal peer proxy downloads: GET /api/peer-file/<ip>/<filename> with header x-peer-llm
                    let is_internal_peer_proxy = path.starts_with("/api/peer-file/")
                        && req.method() == actix_web::http::Method::GET
                        && req.headers().get("x-peer-llm").map(|v| v == "1" || v == "yes").unwrap_or(false);
                    if is_internal_peer_chat || is_internal_peer_file || is_internal_peer_proxy {
                        return Either::Right(srv.call(req));
                    }
                    let ok = req.cookie("session").and_then(|c| {
                        let (_, dk) = jwt_keys(&node_auth_clone.password);
                        decode::<Claims>(c.value(), &dk, &Validation::new(Algorithm::HS256)).ok()
                    }).is_some();
                    if !ok {
                        let resp = HttpResponse::Unauthorized().json(serde_json::json!({"error": "unauthorized"}));
                        return Either::Left(ready(Ok(req.into_response(resp.map_into_boxed_body()))));
                    }
                }
                Either::Right(srv.call(req))
            })
            .wrap_fn(move |req, srv| {
                let path = req.path().to_string();
                let method = req.method().to_string();
                let key = format!("{} {}", method, path);
                let start = Instant::now();
                let state = perf_state_clone.clone();
                let fut = srv.call(req);
                async move {
                    let res = fut.await?;
                    let elapsed = start.elapsed();
                    let ms = elapsed.as_millis() as i64;
                    let resp_status = res.status();
                    {
                        let mut ps = state.lock().await;
                        let entry = ps.per_route.entry(key).or_insert_with(RouteStats::default);
                        entry.durations_ms.push(ms);
                        if entry.durations_ms.len() > 1000 { entry.durations_ms.remove(0); }
                        entry.req_count += 1;
                        if resp_status.as_u16() >= 500 { entry.error_count += 1; }

                        ps.totals.durations_ms.push(ms);
                        if ps.totals.durations_ms.len() > 5000 { ps.totals.durations_ms.remove(0); }
                        ps.totals.req_count += 1;
                        if resp_status.as_u16() >= 500 { ps.totals.error_count += 1; }
                    }
                    Ok(res)
                }
            })
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header()
                .expose_headers(["content-type", "content-length"])
                .max_age(3600)
        )
            .service(web::scope("/api")
                .service(llm::chat)
                .service(upload_file)
                .service(get_files)
                .service(api_status)
                .service(download_file)
                .service(proxy_peer_file)
                .service(analytics_chat)
                .service(analytics_files)
                .service(analytics_engagement)
                .service(analytics_perf)
                .service(analytics_network)
                .service(auth_login)
                .service(auth_status)
                .service(auth_logout))
            .service(get_peers)
            .service(get_local)
            .service(get_index)
            .service(get_root_files)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
