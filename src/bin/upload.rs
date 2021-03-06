#[macro_use]
extern crate log;

use async_std::{
    fs::{remove_file, File},
    io::copy,
};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tide::prelude::*;
use tide::Request;
//
// async fn req_upload(request: Request<_>) {
//
// }
//

#[derive(Clone)]
struct State {
    abs_path: Arc<String>,
    user_agents: Arc<Vec<String>>,
}

impl State {
    fn new(path: String) -> Self {
        let ua = vec![
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/101.0.4951.67 Safari/537.36".into(),
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 12_4) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/101.0.4951.67 Safari/537.36".into(),
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/101.0.4951.67 Safari/537.36".into(),
            "Mozilla/5.0 (Windows NT 10.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/89.0.4389.90 Safari/537.36".into(),
            "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/101.0.4951.67 Safari/537.36".into(),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:100.0) Gecko/20100101 Firefox/100.0".into(),
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 12.4; rv:100.0) Gecko/20100101 Firefox/100.0".into(),
            "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:100.0) Gecko/20100101 Firefox/100.0".into(),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/101.0.4951.67 Safari/537.36 Edg/100.0.1185.39".into(),
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 12_4) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/101.0.4951.67 Safari/537.36 Edg/100.0.1185.39".into(),
        ];
        Self {
            abs_path: Arc::new(path),
            user_agents: Arc::new(ua),
        }
    }

    fn get_user_agent(&self) -> String {
        let mut rng = thread_rng();
        let value = self.user_agents.choose(&mut rng);
        match value {
            Some(v) => v.clone(),
            None => "".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Task {
    path: String,
    link: String,
    filename: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct DelTask {
    url: String,
}

#[derive(Serialize, Debug)]
struct ResTask {
    result: String,
    path: Option<String>,
}

async fn req_v2_hosting_del(mut req: Request<State>) -> tide::Result {
    let DelTask { url } = req.body_json().await?;
    let mut s = false;

    let state = req.state();

    let mut url = Path::new(&url);
    match url.strip_prefix("/images/") {
        Ok(result) => {
            url = result;
        }
        Err(_) => {
            if let Ok(result) = url.strip_prefix("/") {
                url = result;
            }
        }
    }

    // create folder
    let img_path = state.abs_path.as_str();
    let img_path = Path::new(img_path);
    let target_path = img_path.join(&url);

    info!("REQ REMOVE  target path : {:?}", target_path);

    // remove the file
    match remove_file(&target_path).await {
        Ok(_) => {
            info!("... removed {:?}", url);
            s = true;
        }
        Err(err) => {
            error!("Failed to open file {:?} : {:?}", url, err);
        }
    }

    let res = json!(ResTask {
        result: if s {
            String::from("ok")
        } else {
            String::from("error")
        },
        path: None,
    });
    Ok(res.into())
}



use isahc::prelude::*;

async fn get_image_data(user_agent:String, image_url:&str)->Result<Vec<u8>, isahc::Error>{
    let data = isahc::Request::get(image_url).header("user-agent", user_agent).body(()).unwrap().send_async().await?.bytes().await?;
    Ok(data)
}


async fn req_v2_hosting(mut req: Request<State>) -> tide::Result {
    let Task {
        path,
        link,
        filename,
    } = req.body_json().await?;
    info!("received req : {}, {}, {}", path, link, filename);

    let state = req.state();

    // create folder
    let img_path = state.abs_path.as_str();
    let img_path = Path::new(img_path);
    let target_path = img_path.join(&path);

    info!("requested - download path {:?}", target_path);
    // info!("target path {:?}", target_path);
    if let Err(err) = async_std::fs::create_dir_all(&target_path).await {
        error!("failed to create directory {:?}", err);
    }

    let target_path = target_path.join(&filename);

    // download
    let mut s = false;

    let res = get_image_data(state.get_user_agent(), &link).await;
        // surf::get(&link)
        // .header("user-agent", state.get_user_agent())
        // .send()
        // .await;
    match res {
        Ok(res) => {
            let mut dest = File::create(target_path).await?;
            // let content = res.body_bytes().await.unwrap();
            let mut content = &res[..];

            s = copy(&mut content, &mut dest).await.is_ok();
        }
        Err(err) => {
            error!("failed to download {:?} {:?}", target_path, err);
        }
    }

    let new_path = Path::new("/images").join(&path).join(&filename);
    let new_path = new_path.to_str();

    let res = json!(ResTask {
        result: if s {
            String::from("ok")
        } else {
            String::from("error")
        },
        path: if s {
            Some(String::from(new_path.unwrap()))
        } else {
            None
        },
    });

    info!("returning result {:?}", res);
    Ok(res.into())
}

#[derive(Serialize, Debug)]
struct ResStorage {
    pub total: u64,
    pub free: u64,
}

async fn req_storage(mut req: Request<State>) -> tide::Result {
    info!("REQ GET storage");
    let mut res = ResStorage {
        total: 0u64,
        free: 0u64,
    };

    let state = req.state();
    let img_path = state.abs_path.as_str();
    if let Some(path) = std::path::Path::new(img_path).parent(){
        if let Ok(fstats) = fs4::statvfs(path){
            res.total = fstats.total_space();
            res.free = fstats.free_space();
        }
    }

    Ok(json!(res).into())
}

#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    let _ = dotenv::dotenv().ok();
    env_logger::init();
    let img_path = std::env::var("IMAGEHOSTING_PATH").expect("no env var IMAGEHOSTING_PATH");
    info!("local image folder is {}", img_path);

    let mut app = tide::with_state(State::new(img_path.clone()));

    app.at("/v2/hosting")
        .post(req_v2_hosting)
        .delete(req_v2_hosting_del);
    app.at("/v2/storage").get(req_storage);

    app.listen("0.0.0.0:8001").await?;
    Ok(())
}




