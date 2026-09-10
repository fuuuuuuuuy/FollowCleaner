// 网络诊断：验证应用同款 HTTP 客户端（rustls + Chrome UA + 相同超时）能否连通 B站 API。
// 用法：cargo run --example tlstest
// 背景：curl(Windows Schannel) 能通不代表 rustls 能通——部分网络中间件按 TLS 指纹拦截。
use std::time::Duration;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(12))
        .build()
        .expect("build client")
}

#[tokio::main]
async fn main() {
    let c = client();

    println!("[1] passport.bilibili.com  generate（应用二维码生成同款请求）");
    let key = match c
        .get("https://passport.bilibili.com/x/passport-login/web/qrcode/generate")
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            let k = body["data"]["qrcode_key"].as_str().unwrap_or_default().to_string();
            println!("    HTTP {status}  qrcode_key = {}", if k.is_empty() { "<空!>" } else { &k });
            k
        }
        Err(e) => {
            println!("    ERROR: {e}");
            String::new()
        }
    };

    if !key.is_empty() {
        println!("[2] passport.bilibili.com  poll（应用轮询同款请求，期望 code=86101 未扫码）");
        match c
            .get("https://passport.bilibili.com/x/passport-login/web/qrcode/poll")
            .query(&[("qrcode_key", key.as_str())])
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                println!(
                    "    HTTP {status}  data.code={}  message={}",
                    body["data"]["code"].as_i64().unwrap_or(-999),
                    body["data"]["message"].as_str().unwrap_or("?")
                );
            }
            Err(e) => println!("    ERROR: {e}"),
        }
    }

    println!("[3] api.bilibili.com  nav（登录校验同款请求，期望 code=-101 未登录）");
    match c
        .get("https://api.bilibili.com/x/web-interface/nav")
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
            println!(
                "    HTTP {status}  code={}  message={}",
                body["code"].as_i64().unwrap_or(-999),
                body["message"].as_str().unwrap_or("?")
            );
        }
        Err(e) => println!("    ERROR: {e}"),
    }

    println!("[4] 对照组 www.baidu.com（排除整体断网）");
    match c.get("https://www.baidu.com").send().await {
        Ok(r) => println!("    HTTP {}", r.status()),
        Err(e) => println!("    ERROR: {e}"),
    }
}
